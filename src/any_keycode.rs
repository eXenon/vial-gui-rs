// SPDX-License-Identifier: GPL-2.0-or-later
// Port of vial-gui's any_keycode.py — evaluates keycode expressions such as
// "KC_A", "LCTL(KC_A)", "LT(1, KC_A)", "0x1234", "MOD_LCTL | KC_A".

use super::keycodes::KeycodeDb;

pub struct AnyKeycode;

impl AnyKeycode {
    pub fn decode(db: &KeycodeDb, s: &str) -> Result<u32, String> {
        let mut p = Parser {
            db,
            src: s,
            pos: 0,
        };
        let v = p.parse_or()?;
        p.skip_ws();
        if !p.eof() {
            return Err(format!("unexpected trailing characters at {}", p.pos));
        }
        Ok(v)
    }
}

struct Parser<'a> {
    db: &'a KeycodeDb,
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Result<u32, String> {
        let mut v = self.parse_xor()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('|') {
                self.pos += 1;
                v |= self.parse_xor()?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn parse_xor(&mut self) -> Result<u32, String> {
        let mut v = self.parse_and()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('^') {
                self.pos += 1;
                v ^= self.parse_and()?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn parse_and(&mut self) -> Result<u32, String> {
        let mut v = self.parse_primary()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('&') {
                self.pos += 1;
                v &= self.parse_primary()?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn parse_primary(&mut self) -> Result<u32, String> {
        self.skip_ws();
        if self.eof() {
            return Err("unexpected end of expression".to_string());
        }
        match self.peek().unwrap() {
            '(' => {
                self.pos += 1;
                let v = self.parse_or()?;
                if !self.eat(')') {
                    return Err("missing closing parenthesis".to_string());
                }
                Ok(v)
            }
            '0'..='9' => self.parse_number(),
            c if c.is_alphabetic() || c == '_' => {
                let name = self.parse_ident();
                if self.peek_is_lparen() {
                    self.parse_call(&name)
                } else {
                    self.db.any_name(&name).ok_or_else(|| {
                        format!("unknown keycode name '{}'", name)
                    })
                }
            }
            c => Err(format!("unexpected character '{}'", c)),
        }
    }

    fn peek_is_lparen(&self) -> bool {
        let mut i = self.pos;
        while i < self.src.len() {
            let c = self.src[i..].chars().next().unwrap();
            if c.is_whitespace() {
                i += c.len_utf8();
            } else {
                return c == '(';
            }
        }
        false
    }

    fn parse_number(&mut self) -> Result<u32, String> {
        let start = self.pos;
        if self.src[self.pos..].starts_with("0x") || self.src[self.pos..].starts_with("0X") {
            self.pos += 2;
            let nstart = self.pos;
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let text = &self.src[nstart..self.pos];
            u32::from_str_radix(text, 16).map_err(|_| format!("bad hex literal '{}'", text))
        } else {
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let text = &self.src[start..self.pos];
            text.parse::<u32>().map_err(|_| format!("bad number '{}'", text))
        }
    }

    fn parse_ident(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '|' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        self.src[start..self.pos].to_string()
    }

    fn parse_call(&mut self, name: &str) -> Result<u32, String> {
        self.skip_ws();
        if !self.eat('(') {
            return Err(format!("expected '(' after '{}'", name));
        }
        let mut args: Vec<u32> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(')') {
            self.pos += 1;
        } else {
            loop {
                args.push(self.parse_or()?);
                self.skip_ws();
                if self.eat(',') {
                    continue;
                }
                if self.eat(')') {
                    break;
                }
                return Err(format!("expected ',' or ')' in call to '{}'", name));
            }
        }
        eval_function(self.db, name, &args)
            .ok_or_else(|| format!("unknown or invalid keycode function '{}'", name))
    }
}

fn r(db: &KeycodeDb, name: &str) -> u32 {
    db.resolve(name).unwrap_or(0)
}

/// Evaluate a keycode "function" such as LCTL(kc), LT(layer, kc), etc.
fn eval_function(db: &KeycodeDb, name: &str, args: &[u32]) -> Option<u32> {
    let a0 = |args: &[u32]| args.first().copied();
    let a1 = |args: &[u32]| args.get(1).copied();

    let mt = |mods: u32, kc: u32| -> u32 {
        r(db, "QK_MOD_TAP") | ((mods & 0x1F) << 8) | (kc & 0xFF)
    };

    Some(match name {
        "LCTL" | "C" => r(db, "QK_LCTL") | a0(args)?,
        "LSFT" | "S" => r(db, "QK_LSFT") | a0(args)?,
        "LALT" | "LOPT" | "A" => r(db, "QK_LALT") | a0(args)?,
        "LGUI" | "LCMD" | "LWIN" | "G" => r(db, "QK_LGUI") | a0(args)?,
        "RCTL" => r(db, "QK_RCTL") | a0(args)?,
        "RSFT" => r(db, "QK_RSFT") | a0(args)?,
        "RALT" | "ALGR" | "ROPT" => r(db, "QK_RALT") | a0(args)?,
        "RGUI" | "RCMD" | "RWIN" => r(db, "QK_RGUI") | a0(args)?,
        "C_S" => r(db, "QK_LCTL") | r(db, "QK_LSFT") | a0(args)?,
        "HYPR" => r(db, "QK_LCTL") | r(db, "QK_LSFT") | r(db, "QK_LALT") | r(db, "QK_LGUI") | a0(args)?,
        "MEH" => r(db, "QK_LCTL") | r(db, "QK_LSFT") | r(db, "QK_LALT") | a0(args)?,
        "LCAG" => r(db, "QK_LCTL") | r(db, "QK_LALT") | r(db, "QK_LGUI") | a0(args)?,
        "SGUI" | "SCMD" | "SWIN" | "LSG" => r(db, "QK_LGUI") | r(db, "QK_LSFT") | a0(args)?,
        "LCA" => r(db, "QK_LCTL") | r(db, "QK_LALT") | a0(args)?,
        "LSA" => r(db, "QK_LSFT") | r(db, "QK_LALT") | a0(args)?,
        "LAG" => r(db, "QK_LALT") | r(db, "QK_LGUI") | a0(args)?,
        "RSA" | "SAGR" => r(db, "QK_RSFT") | r(db, "QK_RALT") | a0(args)?,
        "RCS" => r(db, "QK_RCTL") | r(db, "QK_RSFT") | a0(args)?,
        "LCG" => r(db, "QK_LCTL") | r(db, "QK_LGUI") | a0(args)?,
        "RCG" => r(db, "QK_RCTL") | r(db, "QK_RGUI") | a0(args)?,
        "LT" => r(db, "QK_LAYER_TAP") | ((a0(args)? & 0xF) << 8) | (a1(args)? & 0xFF),
        "TO" => r(db, "QK_TO") | (r(db, "ON_PRESS") << 4) | (a0(args)? & 0xFF),
        "MO" => r(db, "QK_MOMENTARY") | (a0(args)? & 0xFF),
        "DF" => r(db, "QK_DEF_LAYER") | (a0(args)? & 0xFF),
        "TG" => r(db, "QK_TOGGLE_LAYER") | (a0(args)? & 0xFF),
        "OSL" => r(db, "QK_ONE_SHOT_LAYER") | (a0(args)? & 0xFF),
        "LM" => {
            r(db, "QK_LAYER_MOD")
                | ((a0(args)? & 0xF) << r(db, "QMK_LM_SHIFT"))
                | (a1(args)? & r(db, "QMK_LM_MASK"))
        }
        "OSM" => r(db, "QK_ONE_SHOT_MOD") | (a0(args)? & 0xFF),
        "TT" => r(db, "QK_LAYER_TAP_TOGGLE") | (a0(args)? & 0xFF),
        "MT" => mt(a0(args)?, a1(args)?),
        "TD" => r(db, "QK_TAP_DANCE") | (a0(args)? & 0xFF),
        "LCTL_T" | "CTL_T" => mt(r(db, "MOD_LCTL"), a0(args)?),
        "RCTL_T" => mt(r(db, "MOD_RCTL"), a0(args)?),
        "LSFT_T" | "SFT_T" => mt(r(db, "MOD_LSFT"), a0(args)?),
        "RSFT_T" => mt(r(db, "MOD_RSFT"), a0(args)?),
        "LALT_T" | "LOPT_T" | "ALT_T" | "OPT_T" => mt(r(db, "MOD_LALT"), a0(args)?),
        "RALT_T" | "ROPT_T" | "ALGR_T" => mt(r(db, "MOD_RALT"), a0(args)?),
        "LGUI_T" | "LCMD_T" | "LWIN_T" | "GUI_T" | "CMD_T" | "WIN_T" => mt(r(db, "MOD_LGUI"), a0(args)?),
        "RGUI_T" | "RCMD_T" | "RWIN_T" => mt(r(db, "MOD_RGUI"), a0(args)?),
        "C_S_T" => mt(r(db, "MOD_LCTL") | r(db, "MOD_LSFT"), a0(args)?),
        "MEH_T" => mt(r(db, "MOD_LCTL") | r(db, "MOD_LSFT") | r(db, "MOD_LALT"), a0(args)?),
        "LCAG_T" => mt(r(db, "MOD_LCTL") | r(db, "MOD_LALT") | r(db, "MOD_LGUI"), a0(args)?),
        "RCAG_T" => mt(r(db, "MOD_RCTL") | r(db, "MOD_RALT") | r(db, "MOD_RGUI"), a0(args)?),
        "HYPR_T" | "ALL_T" => mt(
            r(db, "MOD_LCTL") | r(db, "MOD_LSFT") | r(db, "MOD_LALT") | r(db, "MOD_LGUI"),
            a0(args)?,
        ),
        "SGUI_T" | "SCMD_T" | "SWIN_T" | "LSG_T" => mt(r(db, "MOD_LGUI") | r(db, "MOD_LSFT"), a0(args)?),
        "LCA_T" => mt(r(db, "MOD_LCTL") | r(db, "MOD_LALT"), a0(args)?),
        "LSA_T" => mt(r(db, "MOD_LSFT") | r(db, "MOD_LALT"), a0(args)?),
        "LAG_T" => mt(r(db, "MOD_LALT") | r(db, "MOD_LGUI"), a0(args)?),
        "RSA_T" | "SAGR_T" => mt(r(db, "MOD_RSFT") | r(db, "MOD_RALT"), a0(args)?),
        "RCS_T" => mt(r(db, "MOD_RCTL") | r(db, "MOD_RSFT"), a0(args)?),
        "LCG_T" => mt(r(db, "MOD_LCTL") | r(db, "MOD_LGUI"), a0(args)?),
        "RCG_T" => mt(r(db, "MOD_RCTL") | r(db, "MOD_RGUI"), a0(args)?),
        _ => {
            if let Some(rest) = name.strip_prefix("LT") {
                if let Ok(layer) = rest.parse::<u32>() {
                    if layer < 16 {
                        return Some(r(db, "QK_LAYER_TAP") | ((layer & 0xF) << 8) | (a0(args)? & 0xFF));
                    }
                }
            }
            return None;
        }
    })
}
