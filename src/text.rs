/*
pub fn dd_to_int(str: &String , length: usize) -> i32 {
    
    let mut result: i32 = 0;
    let mut bool negative = false;
    let mut i: usize = 0;
    let bstr = str.as_str();

    if str[0] == '-'
    {
        negative = true;
        i = 1; // Consume the - sign
    }
    else
    {
        negative = false;
        i = if str[0] == '+' { 1 } else { 0 }; // Consume a + sign if found
    }

    while (i < length)
    {
        if (str[i] == 0)
            break;
        if (!is_digit(str[i]))
            break;
        result *= 10;
        result += (str[i] - '0');
        ++i;
    }

    return negative ? -result : result;
}
*/
pub fn int_to_dd(mesg: &mut String, argvalue: i32, width: usize, full_sign: bool) {
    let mut value = argvalue;

    if value < 0 {
        mesg.push('-');
        value = -value;
    } else if full_sign {
        mesg.push('+');
    }

    let mut divisor = 1;
    for _i in 0..width - 1 {
        divisor *= 10;
    }

    while divisor >= 1 {
        let digit = value / divisor;
        mesg.push(('0' as u8 + digit as u8) as char);
        value -= digit * divisor;
        divisor /= 10;
    }
}

pub fn charn(c: u8, table_idx: u8) -> char {
    let mut c = c;
    if table_idx != 2 && table_idx != 3 {
        if c == 0 {
            return ' ';
        }
        c -= 1;
    }

    if table_idx != 4 {
        if c < 10 {
            return ('0' as u8 + c) as char;
        }
        c -= 10;
    }

    if table_idx != 3 {
        if c < 26 {
            return ('A' as u8 + c) as char;
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
    } else if table_idx == 5 {
        if c == 0 {
            return '/';
        }
    }

    return '_'; // unknown character, should never get here
}

pub fn is_digit(c: char) -> bool {
    return (c as u8 >= '0' as u8 ) && (c as u8 <= '9' as u8);
}

pub fn is_letter(c: char) -> bool {
    return ((c as u8 >= 'A' as u8) && (c as u8<= 'Z' as u8)) || 
        ((c as u8 >= 'a' as u8) && (c as u8 <= 'z' as u8));
}

pub fn is_space(c: char ) -> bool {
    return c as u8 == ' ' as u8;
}

pub fn in_range(c: char, min: char, max: char) -> bool {
    return (c as u8 >= min as u8) && (c as u8 <= max as u8);
}
