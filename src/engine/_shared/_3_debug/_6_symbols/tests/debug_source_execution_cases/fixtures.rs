use super::*;

// This pinned adapter only maps the fixture/output file protocol to the real native SBPF probe.
// It neither implements nor replaces VM execution, RSP, DWARF or expected observations.
pub(super) const ADAPTER: &str = r#"#!/usr/bin/python3
import hashlib,json,pathlib,subprocess,sys
fixture=json.loads(pathlib.Path(sys.argv[1]).read_text())
def verify(path,pin):
    assert hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()==pin
verify(fixture['probe'],fixture['probeSha256']);verify(fixture['elf'],fixture['elfSha256'])
arguments=[fixture['probe'],fixture['elf'],fixture['mode'],str(fixture['input'])]
if fixture['mode']=='debug':arguments.append(str(fixture['port']))
result=subprocess.run(arguments,stdout=subprocess.PIPE,check=True)
assert len(result.stdout)<=1048576
lines=result.stdout.splitlines();assert len(lines)==2
verify(fixture['probe'],fixture['probeSha256']);verify(fixture['elf'],fixture['elfSha256'])
pathlib.Path(sys.argv[2]).write_text(json.dumps(dict(metadata=json.loads(lines[0]),output=json.loads(lines[1]))))
"#;

pub(super) fn nav(
    symbols: &ExactSymbols,
    client: &mut DebugClient,
    kind: SourceNavigationKind,
    events: &mut Vec<Value>,
) {
    let mut navigation = SourceNavigation::begin(symbols, client, kind).unwrap();
    loop {
        client.step().unwrap();
        let stop = client.wait_stop().unwrap();
        if let Some(outcome) = navigation.observe(symbols, client, &stop).unwrap() {
            assert_eq!(outcome["status"], "complete", "{outcome}");
            events.push(outcome);
            break;
        }
    }
}
