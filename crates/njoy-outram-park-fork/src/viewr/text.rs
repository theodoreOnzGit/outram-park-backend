// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/graph.f90` (`text2`, `text3`, `txtlen`,
// `ssum`, `iget`, `rget`, `charw`, `stripv`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Text with embedded instructions: drawing it (`text3`), measuring it
//! (`txtlen`) and the small decoders they share.
//!
//! The shift characters are `<` (upper-case standard), `>` (lower-case or
//! mixed standard), `[` (upper-case Greek), `]` (lower/mixed Greek) and `#`
//! (instructions `Ev`, `Lv`, `Hv`, `Fi`, `Mi`, `X`); a shift character given
//! twice stands for itself. Strings are bytes, as Fortran `character`.
//!
//! **Fortran behaviour kept on purpose.** `txtlen` sums `temp(1:l)` where `l`
//! is the last non-blank position of the pending text; `l` keeps its old value
//! after the text is flushed, as upstream's local does, and is 0 before any text
//! (upstream leaves it undefined there, and no string NJOY generates reaches
//! that case). `text3` reads `txt(i+1:i+1)` one past `ntxt` at the end of a
//! string: here a blank, which is what the blank-padded 80-column variables hold.

use petir::mathf::RealMath;

use super::graph::Graph;

/// `txt(i:i)` with `i` 1-based; a blank past the end.
fn ch(txt: &[u8], i: i32) -> u8 {
    if i >= 1 && (i as usize) <= txt.len() {
        txt[i as usize - 1]
    } else {
        b' '
    }
}

fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

/// `temp = text` for an 80-character variable: the text, blank padded.
fn set_temp(temp: &mut [u8], text: &[u8]) {
    temp.fill(b' ');
    temp[..text.len()].copy_from_slice(text);
}

/// `iget(strng,n)` (`graph.f90:2033`): the integer in the first `n` characters
/// (digits accumulate, `-` negates, anything else is skipped).
pub fn iget(strng: &[u8], n: usize) -> i32 {
    let mut neg = false;
    let mut ival = 0_i32;
    for &c in strng.iter().take(n) {
        if is_digit(c) {
            ival = ival.wrapping_mul(10).wrapping_add(i32::from(c - b'0'));
        } else if c == b'-' {
            neg = true;
        }
    }
    if neg {
        -ival
    } else {
        ival
    }
}

/// `rget(strng,n)` (`graph.f90:2061`): the real in the first `n` characters; the
/// digits after a `.` scale by `10**(1-ipwr)` (integer power, libgcc's).
pub fn rget(strng: &[u8], n: usize) -> f64 {
    let mut neg = false;
    let mut ival = 0_i32;
    let mut ipwr = 0_i32;
    for &c in strng.iter().take(n) {
        if is_digit(c) {
            ival = ival.wrapping_mul(10).wrapping_add(i32::from(c - b'0'));
            if ipwr != 0 {
                ipwr += 1;
            }
        } else if c == b'.' {
            ipwr = 1;
        } else if c == b'-' {
            neg = true;
        }
    }
    let mut val = f64::from(ival);
    if ipwr != 0 {
        val *= 10.0_f64.r_powi(1 - ipwr);
    }
    if neg {
        -val
    } else {
        val
    }
}

/// `stripv(str,length)` (`graph.f90:2865`): strip leading and trailing
/// characters outside codes 33-126 and return the remainder with its count.
pub fn stripv(s: &[u8]) -> (Vec<u8>, usize) {
    let mut j = 0usize;
    let mut k = 0usize;
    for (idx, &c) in s.iter().enumerate() {
        if c > 32 && c < 127 {
            if j == 0 {
                j = idx + 1;
            }
            k = idx + 1;
        }
    }
    if j == 0 {
        return (Vec::new(), 0);
    }
    (s[j - 1..k].to_vec(), k - j + 1)
}

impl Graph {
    /// `charw(char,itable)` (`graph.f90:2850`): the width of a character in font
    /// `itable` (1-3); 0 outside the table.
    pub fn charw(&self, c: u8, itable: i32) -> f64 {
        if (1..=3).contains(&itable) && c < 128 {
            self.cw[itable as usize - 1][c as usize]
        } else {
            0.0
        }
    }

    /// `ssum(temp,l,ifont,ialf,scale)` (`graph.f90:2013`): summed width of the
    /// first `l` characters.
    pub fn ssum(&self, temp: &[u8], l: usize, ifont: i32, scale: f64) -> f64 {
        let mut s = 0.0_f64;
        for &c in temp.iter().take(l) {
            s += scale * self.charw(c, ifont);
        }
        s
    }

    /// `text2(txt,ntxt,ifont,ht,xo,yo,xx,yx,xy,yy)` (`graph.f90:1634`).
    #[allow(clippy::too_many_arguments)] // upstream's signature
    pub fn text2(&mut self, txt: &[u8], ntxt: i32, ifont: i32, ht: f64, xo: f64, yo: f64, xx: f64, yx: f64, xy: f64, yy: f64) {
        self.text3(txt, ntxt, ifont, ht, xo, yo, 0.0, xx, yx, 0.0, xy, yy, 0.0);
    }

    /// `text3(txt,ntxt,ifont,ht,xo,yo,zo,xx,yx,zx,xy,yy,zy)` (`graph.f90:1649`):
    /// draw a string with instructions in 3-d perspective.
    #[allow(clippy::too_many_arguments)] // upstream's signature
    pub fn text3(
        &mut self,
        txt: &[u8],
        ntxt: i32,
        ifont: i32,
        ht: f64,
        xo: f64,
        yo: f64,
        zo: f64,
        xx: f64,
        yx: f64,
        zx: f64,
        xy: f64,
        yy: f64,
        zy: f64,
    ) {
        let mut scale = 1.0_f64;
        let (mut x, mut y, mut z) = (xo, yo, zo);
        let mut delta = 0.0_f64;
        let mut cbase = 0.0_f64;
        let mut celev;
        let mut icase = 0;
        let mut ialf = 0;
        let mut imode = 0;
        let mut jfont = ifont;
        let mut i = 1_i32;
        let mut j = 0usize;
        let mut comm = b' ';
        let mut temp = [b' '; 80];
        while i <= ntxt {
            let mut c = ch(txt, i);
            if imode == 1 {
                if is_digit(c) || c == b'.' || c == b'-' || c == b'X' {
                    if j < 80 {
                        temp[j] = c;
                    }
                    j += 1;
                } else {
                    if j == 0 {
                        temp[0] = b'D';
                        j = 1;
                    }
                    if comm == b'E' {
                        if temp[0] == b'D' {
                            set_temp(&mut temp, b".5");
                            j = 2;
                        }
                        if temp[0] == b'X' {
                            celev = 0.0;
                        } else {
                            let dy = rget(&temp, j);
                            celev = if dy > 0.0 { ht * dy } else { -ht * dy };
                        }
                        delta = celev;
                    } else if comm == b'L' {
                        if temp[0] == b'D' {
                            set_temp(&mut temp, b".5");
                            j = 2;
                        }
                        if temp[0] == b'X' {
                            celev = 0.0;
                        } else {
                            let dy = rget(&temp, j);
                            celev = if dy > 0.0 { -ht * dy } else { ht * dy };
                        }
                        delta = celev;
                    } else if comm == b'H' {
                        if temp[0] == b'D' {
                            set_temp(&mut temp, b".7");
                            j = 2;
                        }
                        if temp[0] == b'X' {
                            scale = 1.0;
                        } else {
                            scale = rget(&temp, j);
                        }
                    } else if comm == b'O' {
                        let dy = rget(&temp, j);
                        cbase += dy;
                    } else if comm == b'F' {
                        jfont = iget(&temp, j);
                    } else if comm == b'M' {
                        ialf = iget(&temp, j);
                    }
                    comm = c;
                    j = 0;
                    if c == b'<' || c == b'>' || c == b'[' || c == b']' {
                        imode = 0;
                        i -= 1;
                    }
                }
            } else {
                let mut ins = false;
                let mut iskip = false;
                if c == b'#' {
                    if ch(txt, i + 1) == b'#' {
                        i += 1;
                    } else {
                        ins = true;
                    }
                }
                if c == b'<' {
                    if ch(txt, i + 1) == b'<' {
                        i += 1;
                    } else {
                        jfont = ifont;
                        icase = 1;
                        iskip = true;
                    }
                }
                if c == b'>' {
                    if ch(txt, i + 1) == b'>' {
                        i += 1;
                    } else {
                        jfont = ifont;
                        icase = 0;
                        iskip = true;
                    }
                }
                if c == b'[' {
                    if ch(txt, i + 1) == b'[' {
                        i += 1;
                    } else {
                        jfont = 3;
                        icase = 1;
                        iskip = true;
                    }
                }
                if c == b']' {
                    if ch(txt, i + 1) == b']' {
                        i += 1;
                    } else {
                        jfont = 3;
                        icase = 0;
                        iskip = true;
                    }
                }
                if ins {
                    imode = 1;
                    comm = c;
                    j = 0;
                } else if iskip {
                    // a shift character: nothing is drawn
                } else {
                    if icase == 1 {
                        let mut ii = i32::from(c);
                        if ii >= 96 {
                            ii -= 32;
                        }
                        c = ii as u8;
                    }
                    let siz = scale * ht;
                    let wi = self.ssum(&[c], 1, jfont, siz);
                    let xn = x + delta * xy;
                    let yn = y + delta * yy;
                    let zn = z + delta * zy;
                    let (wu, wv) = self.trans3(xn, yn, zn);
                    let (u, v) = self.transw(wu, wv);
                    let (wu, wv) = self.trans3(xn + xx, yn + yx, zn + zx);
                    let (mut ux, mut vx) = self.transw(wu, wv);
                    let (wu, wv) = self.trans3(xn + xy, yn + yy, zn + zy);
                    let (mut uy, mut vy) = self.transw(wu, wv);
                    ux -= u;
                    vx -= v;
                    uy -= u;
                    vy -= v;
                    self.dchr(u, v, c, jfont, ialf, siz, ux, vx, uy, vy);
                    x += wi * xx;
                    y += wi * yx;
                    z += wi * zx;
                }
            }
            i += 1;
        }
        let _ = cbase;
    }

    /// `txtlen(txt,ntxt,ifont,ht)` (`graph.f90:1838`): the length of a string
    /// with instructions, `ifont` and `ht` those of the normal characters.
    pub fn txtlen(&self, txt: &[u8], ntxt: i32, ifont: i32, ht: f64) -> f64 {
        let mut siz = 0.0_f64;
        let mut icase = 0;
        let mut ialf = 0;
        let mut scale = 1.0_f64;
        let mut imode = 0;
        let mut jfont = ifont;
        let mut i = 1_i32;
        let mut j = 0usize;
        let mut l = 0usize;
        let mut comm = b' ';
        let mut temp = [b' '; 400];
        while i <= ntxt {
            let mut c = ch(txt, i);
            if imode == 1 {
                if is_digit(c) || c == b'.' || c == b'-' || c == b'X' {
                    if j < 400 {
                        temp[j] = c;
                    }
                    j += 1;
                } else {
                    if j == 0 {
                        temp[0] = b'D';
                        j = 1;
                    }
                    let t = &mut temp;
                    if comm == b'E' {
                        if t[0] == b'D' {
                            set_temp(t, b".5");
                            j = 2;
                        }
                        // `celev` is computed upstream and never used by `txtlen`.
                        if t[0] != b'X' {
                            let _ = rget(t, j);
                        }
                    } else if comm == b'L' {
                        if t[0] == b'D' {
                            set_temp(t, b".5");
                            j = 2;
                        }
                        if t[0] != b'X' {
                            let _ = rget(t, j);
                        }
                    } else if comm == b'H' {
                        if t[0] == b'D' {
                            set_temp(t, b".7");
                            j = 2;
                        }
                        if t[0] == b'X' {
                            scale = 1.0;
                        } else {
                            scale = rget(t, j);
                        }
                    } else if comm == b'O' {
                        let _ = rget(t, j);
                    } else if comm == b'F' {
                        jfont = iget(t, j);
                    } else if comm == b'M' {
                        ialf = iget(t, j);
                    }
                    comm = c;
                    j = 0;
                    if c == b'<' || c == b'>' || c == b'[' || c == b']' {
                        imode = 0;
                    }
                }
            } else {
                let mut ins = false;
                let mut iskip = false;
                if i < ntxt {
                    if c == b'#' {
                        if ch(txt, i + 1) == b'#' {
                            i += 1;
                        } else {
                            ins = true;
                        }
                    }
                    if c == b'<' {
                        if ch(txt, i + 1) == b'<' {
                            i += 1;
                        } else {
                            icase = 1;
                            jfont = ifont;
                            iskip = true;
                        }
                    }
                    if c == b'>' {
                        if ch(txt, i + 1) == b'>' {
                            i += 1;
                        } else {
                            icase = 0;
                            jfont = ifont;
                            iskip = true;
                        }
                    }
                    if c == b'[' {
                        if ch(txt, i + 1) == b'[' {
                            i += 1;
                        } else {
                            icase = 1;
                            jfont = 3;
                            iskip = true;
                        }
                    }
                    if c == b']' {
                        if ch(txt, i + 1) == b']' {
                            i += 1;
                        } else {
                            icase = 0;
                            jfont = 3;
                            iskip = true;
                        }
                    }
                }
                if ins {
                    imode = 1;
                    siz += self.ssum(&temp, l, jfont, scale);
                    comm = c;
                    j = 0;
                } else if iskip {
                    // a shift character: no width
                } else if ialf == 0 {
                    j += 1;
                    if icase == 1 {
                        let mut ii = i32::from(c);
                        if ii >= 96 {
                            ii -= 32;
                        }
                        c = ii as u8;
                    }
                    if j <= 400 {
                        temp[j - 1] = c;
                    }
                    if c != b' ' {
                        l = j;
                    }
                } else {
                    j += 1;
                    let esc = format!("\\{}", super::fmt::o3(u32::from(c) + 128));
                    for (k, b) in esc.bytes().enumerate() {
                        if j - 1 + k < 400 {
                            temp[j - 1 + k] = b;
                        }
                    }
                    j += 3;
                    l = j;
                }
            }
            i += 1;
        }
        if j > 0 {
            siz += self.ssum(&temp, l, jfont, scale);
        }
        siz * ht
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g() -> Graph {
        let mut g = Graph::default();
        g.gplot(1, 10.0, 7.5);
        g.initp(1, 10.0, 7.5, 2, 0.3, 0.0134, 0, 0);
        g
    }

    #[test]
    fn decoders() {
        assert_eq!(iget(b"-12", 3), -12);
        assert_eq!(iget(b"3", 1), 3);
        assert_eq!(rget(b".8", 2), 8.0 * 0.1);
        assert_eq!(rget(b"1.25", 4), 125.0 * 10.0_f64.r_powi(-2));
        assert_eq!(stripv(b"  ab c  ").0, b"ab c".to_vec());
        assert_eq!(stripv(b"   ").1, 0);
    }

    #[test]
    fn plain_text_width_is_the_sum_of_the_table() {
        let g = g();
        // Helvetica: 'A' .667, 'b' .556.
        let w = g.txtlen(b"Ab", 2, 2, 0.3);
        assert_eq!(w, (1.0 * 0.667 + 1.0 * 0.556) * 0.3);
    }

    #[test]
    fn shift_characters_and_instructions_have_no_width() {
        let g = g();
        // "<c>ross": '<' shifts to upper case standard, '>' back; the width is C + ross.
        let w1 = g.txtlen(b"<c>ross", 7, 2, 1.0);
        let w2 = g.txtlen(b"Cross", 5, 2, 1.0);
        assert_eq!(w1, w2);
        // log-axis label "10#EH.8<3#HXEX<" measures the digits only (scaled).
        let w = g.txtlen(b"10#EH.8<3#HXEX<", 15, 2, 1.0);
        assert!(w > g.txtlen(b"10", 2, 2, 1.0));
    }

    #[test]
    fn text3_draws_one_show_per_visible_character() {
        let mut g = g();
        let n0 = g.out.matches(" show").count();
        g.text2(b"<e>nergy (<m>e<v>)", 18, 2, 0.3, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0);
        // E n e r g y ' ' ( M e V ) : 12 visible characters.
        assert_eq!(g.out.matches(" show").count() - n0, 12);
        assert!(g.out.contains("(E) show") && g.out.contains("(M) show") && g.out.contains("(\\() show"));
    }
}
