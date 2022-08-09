pub fn dd_to_int(str: &str) -> i32 {
    str.parse().unwrap()
}

pub fn int_to_dd(mesg: &mut String, value: i32, full_sign: bool) {
    if full_sign {
        mesg.push_str(&format!("{:+2}",value).to_string());
    } else {
        mesg.push_str(&format!("{:2}",value).to_string());
    }
}

pub fn charn(mut c: u8, table_idx: u8) -> char {
    if table_idx != 2 && table_idx != 3 {
        if c == 0 {
            return ' ';
        }
        c -= 1;
    }

    if table_idx != 4 {
        if c < 10 {
            return (b'0' + c) as char;
        }
        c -= 10;
    }

    if table_idx != 3 {
        if c < 26 {
            return (b'A' + c) as char;
        }
        c -= 26;
    }

    if table_idx == 0 {
        if c < 5 {
            match c {
                0 => return '+',
                1 => return '-',
                2 => return '.',
                3 => return '/',
                4 => return '?',
                _ => return '_',
            }
        }
    } else if table_idx == 5 && c == 0 {
        return '/';
    }
    '_' // unknown character, should never get here
}

pub fn in_range(c: char, min: char, max: char) -> bool {
    (c as u8 >= min as u8) && (c as u8 <= max as u8)
}
