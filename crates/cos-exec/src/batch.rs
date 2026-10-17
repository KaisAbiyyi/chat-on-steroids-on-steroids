use base64::{Engine, engine::general_purpose::STANDARD};
pub fn compose(commands: &[String], shell: &str, marker: &str) -> String {
    if shell == "powershell" {
        let encoded = commands
            .iter()
            .map(|c| format!("'{}'", STANDARD.encode(c)))
            .collect::<Vec<_>>()
            .join(", ");
        return format!(
            r#"$__cos_batch_commands = @({encoded})
$__cos_batch_exit = 0
for ($__cos_batch_index = 0; $__cos_batch_index -lt $__cos_batch_commands.Count; $__cos_batch_index++) {{
  [Console]::Out.WriteLine(("--- command {{0}}/{{1}} --- [clf-batch:{marker}]" -f ($__cos_batch_index + 1), $__cos_batch_commands.Count))
  $global:LASTEXITCODE = 0
  try {{
    $__cos_batch_text = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($__cos_batch_commands[$__cos_batch_index]))
    . ([ScriptBlock]::Create($__cos_batch_text))
    $__cos_batch_succeeded = $?
    $__cos_batch_code = if ($LASTEXITCODE -ne 0) {{ [int]$LASTEXITCODE }} elseif ($__cos_batch_succeeded) {{ 0 }} else {{ 1 }}
  }} catch {{
    [Console]::Error.WriteLine($_.ToString())
    $__cos_batch_code = 1
  }}
  [Console]::Out.WriteLine(("--- exit code {{0}} --- [clf-batch:{marker}]" -f $__cos_batch_code))
  if ($__cos_batch_code -ne 0 -and $__cos_batch_exit -eq 0) {{ $__cos_batch_exit = $__cos_batch_code }}
}}
exit $__cos_batch_exit"#
        );
    }
    if shell == "cmd" {
        let mut lines = vec!["@echo off".into(), "set \"__cos_batch_exit=0\"".into()];
        for (i, command) in commands.iter().enumerate() {
            lines.extend([format!("echo --- command {}/{} --- [clf-batch:{marker}]",i+1,commands.len()),format!("call {command}"),"set \"__cos_batch_code=%errorlevel%\"".into(),format!("echo --- exit code %__cos_batch_code% --- [clf-batch:{marker}]"),"if not \"%__cos_batch_code%\"==\"0\" if \"%__cos_batch_exit%\"==\"0\" set \"__cos_batch_exit=%__cos_batch_code%\"".into()]);
        }
        lines.push("exit /b %__cos_batch_exit%".into());
        return lines.join("\r\n");
    }
    let mut lines = vec!["__cos_batch_exit=0".into()];
    for (i, command) in commands.iter().enumerate() {
        lines.extend([format!("printf '%s\\n' {}",crate::ssh::quote(&format!("--- command {}/{} --- [clf-batch:{marker}]",i+1,commands.len()))),format!("eval {}",crate::ssh::quote(command)),"__cos_batch_code=$?".into(),format!("printf '%s\\n' \"--- exit code $__cos_batch_code --- [clf-batch:{marker}]\""),"if [ \"$__cos_batch_code\" -ne 0 ] && [ \"$__cos_batch_exit\" -eq 0 ]; then __cos_batch_exit=$__cos_batch_code; fi".into()]);
    }
    lines.push("exit \"$__cos_batch_exit\"".into());
    lines.join("\n")
}
pub struct Display {
    delimiter: Vec<u8>,
    pending: Vec<u8>,
}
impl Display {
    pub fn new(marker: &str) -> Self {
        Self {
            delimiter: format!(" [clf-batch:{marker}]").into_bytes(),
            pending: vec![],
        }
    }
    pub fn push(&mut self, chunk: &[u8], final_chunk: bool) -> Vec<u8> {
        let mut input = std::mem::take(&mut self.pending);
        input.extend_from_slice(chunk);
        let mut out = vec![];
        let mut at = 0;
        while at < input.len() {
            if input[at..].starts_with(&self.delimiter) {
                at += self.delimiter.len();
                continue;
            }
            if !final_chunk && self.delimiter.starts_with(&input[at..]) {
                self.pending.extend_from_slice(&input[at..]);
                break;
            }
            out.push(input[at]);
            at += 1;
        }
        out
    }
}
