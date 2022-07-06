pub fn int_to_dd(mesg: &mut String, argvalue: i32, width: usize, full_sign: bool)
{
    let mut value = argvalue;

    if value < 0 {
        mesg.push('-');
        value = -value;
    }
    else if full_sign {
        mesg.push('+');
    }

    let mut divisor = 1;
    for _i in 0..width-1 {
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
        if c == 0 { return ' ';}
        c -= 1;
    }

    if table_idx != 4 {
        if c < 10 { return ('0' as u8 + c) as char; }
        c -= 10;
    }

    if table_idx != 3 {
        if c < 26 { return ('A' as u8 + c) as char }
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
    }
    else if table_idx == 5 {
        if c == 0 {return '/'}
    }

    return '_'; // unknown character, should never get here
}