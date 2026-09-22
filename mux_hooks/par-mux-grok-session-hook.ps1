# installed by par-term
# managed by par-term; reinstalling or updating the integration overwrites
# this file. add custom hooks beside this file instead of editing it.
# PowerShell port of the POSIX reporter (par-mux-grok-session-hook.sh):
# one JSON line over the control socket, one reply read, close. Inert
# outside a par-mux pane (env guards) and silent on every failure.
# PAR_MUX_INTEGRATION_ID=grok
# PAR_MUX_INTEGRATION_VERSION=1

param([string]$Action = "")

if ($Action -ne "session") { exit 0 }
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
    $null = $socket.Send($Bytes)
    $buffer = New-Object byte[] 4096
    $null = $socket.Receive($buffer)
    $socket.Close()
}

$inputText = [Console]::In.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { $null } else { $inputText | ConvertFrom-Json }
} catch {
    exit 0
}

# Backstop: only report on session-start payloads. Grok emits
# "session_start" and accepts the Claude/Cursor spellings, so tolerate all
# three; a missing field is allowed for forward compatibility.
if ($null -ne $payload) {
    $hookEventName = $payload.hook_event_name
    if ($null -ne $hookEventName -and $hookEventName -is [string] -and $hookEventName -notin @("session_start", "SessionStart", "sessionStart")) { exit 0 }
}

$source = if ($null -ne $payload) { $payload.source } else { $null }

# Grok injects GROK_SESSION_ID into every hook process; prefer it and fall
# back to the event payload's session id fields.
$sessionId = $env:GROK_SESSION_ID
if ([string]::IsNullOrWhiteSpace($sessionId) -and $null -ne $payload) {
    $sessionId = $payload.session_id
    if ([string]::IsNullOrWhiteSpace("$sessionId")) {
        $sessionId = $payload.sessionId
    }
}
if ([string]::IsNullOrWhiteSpace("$sessionId")) { exit 0 }

$seq = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() * 1000000
$requestId = "par-mux:grok:$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()):$('{0:D6}' -f (Get-Random -Minimum 0 -Maximum 999999))"
$params = [ordered]@{
    pane_id = "$($env:PAR_MUX_PANE_ID)"
    source = "par-mux:grok"
    agent = "grok"
    seq = $seq
    agent_session_id = "$sessionId"
}
if (-not [string]::IsNullOrWhiteSpace($source)) {
    $params["session_start_source"] = "$source"
}

$request = [ordered]@{
    id = $requestId
    method = "pane.report_agent_session"
    params = $params
} | ConvertTo-Json -Compress -Depth 6

try {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($request + "`n")
    Send-MuxReport $env:PAR_MUX_SOCKET $bytes
} catch {
}
exit 0
