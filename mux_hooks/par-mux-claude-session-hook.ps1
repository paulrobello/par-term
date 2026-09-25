# par-mux claude session hook (Windows) — reports session identity to the
# par-mux daemon on claude SessionStart events (no action argument, or
# `session`) and releases the pane claim on SessionEnd (`release`).
# PowerShell port of the POSIX reporter (par-mux-claude-session-hook.sh):
# one JSON line over the control socket, one reply read, close. Inert
# outside a par-mux pane (env guards) and silent on every failure.
# PAR_MUX_INTEGRATION_ID=claude
# PAR_MUX_INTEGRATION_VERSION=2

if ($env:PAR_MUX_ENV -ne "1") { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:PAR_MUX_SOCKET)) { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:PAR_MUX_PANE_ID)) { exit 0 }

# Transport mirrors the daemon's platform split (par-term-emu-core-rust
# mux ipc.rs): named pipes on Windows - the socket path string becomes the
# pipe name after \\.\pipe\, exactly as the interprocess crate derives it -
# and Unix-domain sockets elsewhere (pwsh 7 always has
# UnixDomainSocketEndPoint; .NET Framework, which lacks it, only runs on
# Windows, whose arm this is). One JSON line out, one reply read, close.
function Send-MuxReport([string]$Path, [byte[]]$Bytes) {
    if ($env:OS -eq "Windows_NT") {
        $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(
            ".", $Path, [System.IO.Pipes.PipeDirection]::InOut)
        $pipe.Connect(500)
        $pipe.Write($Bytes, 0, $Bytes.Length)
        $buffer = New-Object byte[] 4096
        $null = $pipe.Read($buffer, 0, $buffer.Length)
        $pipe.Close()
        return
    }
    $socket = [System.Net.Sockets.Socket]::new(
        [System.Net.Sockets.AddressFamily]::Unix,
        [System.Net.Sockets.SocketType]::Stream,
        [System.Net.Sockets.ProtocolType]::Unspecified)
    $socket.SendTimeout = 500
    $socket.ReceiveTimeout = 500
    $socket.Connect([System.Net.Sockets.UnixDomainSocketEndPoint]::new($Path))
    $null = $socket.Send($bytes)
    $buffer = New-Object byte[] 4096
    $null = $socket.Receive($buffer)
    $socket.Close()
}

$action = if ($args.Count -gt 0) { "$($args[0])" } else { "" }
$seq = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() * 1000000

if ($action -eq "release") {
    # pane.release_agent clears the pane claim (core aed41c2). The guards
    # are on the daemon side; a later timestamp always advances past the
    # session report seq from the same source.
    $request = [ordered]@{
        id = "par-mux:claude:release:$seq"
        method = "pane.release_agent"
        params = [ordered]@{
            pane_id = "$($env:PAR_MUX_PANE_ID)"
            source = "par-mux:claude"
            agent = "claude"
            seq = $seq
        }
    } | ConvertTo-Json -Compress -Depth 6
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($request + "`n")
        Send-MuxReport $env:PAR_MUX_SOCKET $bytes
    } catch {
    }
    exit 0
}

$inputText = [Console]::In.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { $null } else { $inputText | ConvertFrom-Json }
} catch {
    exit 0
}

$sessionId = $payload.session_id
if ([string]::IsNullOrWhiteSpace($sessionId)) { exit 0 }

$source = $payload.source
if ([string]::IsNullOrWhiteSpace($source)) { $source = "startup" }

$params = [ordered]@{
    pane_id = "$($env:PAR_MUX_PANE_ID)"
    source = "par-mux:claude"
    agent = "claude"
    seq = $seq
    session_start_source = "$source"
    agent_session_id = "$sessionId"
    session_resume_argv = @("claude", "--resume", "$sessionId")
}
if ($payload.transcript_path -is [string] -and -not [string]::IsNullOrWhiteSpace($payload.transcript_path)) {
    $params["agent_session_path"] = "$($payload.transcript_path)"
}

$request = [ordered]@{
    id = "par-mux:claude:$seq"
    method = "pane.report_agent_session"
    params = $params
} | ConvertTo-Json -Compress -Depth 6

try {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($request + "`n")
    Send-MuxReport $env:PAR_MUX_SOCKET $bytes
} catch {
}
exit 0
