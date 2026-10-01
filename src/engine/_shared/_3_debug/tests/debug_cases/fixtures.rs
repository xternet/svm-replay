use super::*;

pub(super) fn receive(stream: &mut TcpStream) -> String {
    let mut byte = [0];
    loop {
        stream.read_exact(&mut byte).expect("command prefix");
        if byte[0] == b'$' {
            break;
        }
        assert_eq!(byte[0], b'+');
    }
    let mut command = Vec::new();
    loop {
        stream.read_exact(&mut byte).expect("command byte");
        if byte[0] == b'#' {
            break;
        }
        command.push(byte[0]);
    }
    let mut checksum = [0; 2];
    stream.read_exact(&mut checksum).expect("command checksum");
    assert_eq!(
        String::from_utf8(checksum.to_vec()).expect("hex"),
        format!(
            "{:02x}",
            command.iter().fold(0_u8, |sum, b| sum.wrapping_add(*b))
        )
    );
    String::from_utf8(command).expect("ASCII command")
}

pub(super) fn respond(stream: &mut TcpStream, response: &str) {
    let sum = response.bytes().fold(0_u8, |sum, b| sum.wrapping_add(b));
    stream
        .write_all(format!("+${response}#{sum:02x}").as_bytes())
        .expect("response");
}

pub(super) fn budget() -> ExecutionBudget {
    ExecutionBudget::new(Duration::from_secs(3), CancellationToken::new()).expect("budget")
}
