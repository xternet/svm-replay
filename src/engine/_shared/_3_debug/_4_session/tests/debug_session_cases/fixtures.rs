use super::*;

// A live, nested RSP protocol fixture. This is not an SVM/parity oracle.
pub(super) const WORKER: &str = r#"#!/usr/bin/python3
import json,pathlib,select,socket,sys
port=int(sys.argv[3]); program='11111111111111111111111111111111'
def invoke(index,ancestors):
    listener=socket.socket();listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);listener.bind(('127.0.0.1',port));listener.listen(1)
    print('Waiting for bounded debugger on 127.0.0.1:'+str(port),file=sys.stderr,flush=True)
    stream,peer=listener.accept();listener.close();assert peer[0]=='127.0.0.1'
    pc=0x1000;breakpoints=set();nested=False
    def send(text):
        raw=text.encode();stream.sendall(b'$'+raw+b'#'+format(sum(raw)%256,'02x').encode())
    def receive():
        while True:
            byte=stream.recv(1)
            assert byte, 'debugger disconnected'
            if byte==b'\x03':return 'interrupt'
            if byte==b'$':break
            assert byte==b'+'
        raw=b''
        while True:
            byte=stream.recv(1);assert byte
            if byte==b'#':break
            raw+=byte
        checksum=stream.recv(2);assert int(checksum,16)==sum(raw)%256
        stream.sendall(b'+');return raw.decode()
    while True:
        command=receive()
        if command.startswith('qRcmd,'):
            assert bytes.fromhex(command[6:])==b'metadata'
            metadata='execution_mode=interpreter-debug;execution_index=0;program_id='+program+';invocation_index='+str(index)+';cpi_level='+str(len(ancestors))+';caller_index='+('none' if not ancestors else str(ancestors[-1]))+';caller='+('none' if not ancestors else program)+';ancestors='+('none' if not ancestors else ','.join(map(str,ancestors)))+';elf_sha256='+'a'*64
            send('O'+metadata.encode().hex());send('OK')
        elif command=='g':send(b''.join(r.to_bytes(8,'little') for r in [0]*11+[pc]).hex())
        elif command.startswith(('Z0,','z0,')):
            value=int(command.split(',')[1],16)
            if command.startswith('Z'):breakpoints.add(value)
            else:breakpoints.remove(value)
            send('OK')
        elif command in ['s','c']:
            while True:
                if command=='c' and select.select([stream],[],[],0)[0]:
                    interrupt=stream.recv(1)
                    if interrupt==b'+':continue
                    assert interrupt==b'\x03';send('S02');break
                if index==0 and not nested:
                    nested=True;invoke(1,[0])
                pc+=8
                if pc>=0x1020:send('W00');stream.close();return
                if pc in breakpoints or command=='s':send('S05');break
        else:raise AssertionError('unexpected command '+command)
invoke(0,[])
pathlib.Path(sys.argv[2]).write_text(json.dumps(dict(computed='all live fixture VMs exited')))
"#;

pub(super) fn setup(root: &std::path::Path) -> (WorkerSpec, u16) {
    let executable = root.join("debug-worker");
    fs::write(&executable, WORKER).expect("protocol fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).expect("mode");
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve loopback");
    let port = listener.local_addr().expect("endpoint").port();
    drop(listener);
    (
        WorkerSpec {
            sha256: file_sha256(&executable).expect("pin"),
            executable,
        },
        port,
    )
}

pub(super) fn target(event: &Value) -> PauseToken {
    serde_json::from_value(event["target"].clone()).expect("actual target token")
}
