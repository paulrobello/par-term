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

# Unix-domain socket endpoint: UnixDomainSocketEndPoint on pwsh 7 (.NET
# Core); Windows PowerShell 5.1's .NET Framework lacks the type, but AF_UNIX
# CLIENT connect works there with a hand-rolled sockaddr_un endpoint (the
# classic Mono UnixEndPoint shape — connect-only, which is all a hook needs).
function Get-MuxSocketEndpoint([string]$Path) {
    try { return [System.Net.Sockets.UnixDomainSocketEndPoint]::new($Path) } catch {}
    if (-not ('UnixEndPoint' -as [type])) {
        Add-Type -TypeDefinition @'
using System; using System.Net; using System.Net.Sockets; using System.Text;
public class UnixEndPoint : EndPoint {
    private string path;
    public UnixEndPoint(string path) { this.path = path; }
    public override AddressFamily AddressFamily { get { return AddressFamily.Unix; } }
    public override SocketAddress Serialize() {
        byte[] bytes = Encoding.UTF8.GetBytes(path);
        SocketAddress sa = new SocketAddress(AddressFamily.Unix, bytes.Length + 3);
        for (int i = 0; i < bytes.Length; i++) sa[2 + i] = bytes[i];
        return sa;
    }
    public override EndPoint Create(SocketAddress socketAddress) { throw new NotImplementedException(); }
}
'@
    }
    [UnixEndPoint]::new($Path)
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
    $socket = [System.Net.Sockets.Socket]::new(
        [System.Net.Sockets.AddressFamily]::Unix,
        [System.Net.Sockets.SocketType]::Stream,
        [System.Net.Sockets.ProtocolType]::Unspecified)
    $socket.SendTimeout = 500
    $socket.ReceiveTimeout = 500
    $socket.Connect((Get-MuxSocketEndpoint $env:PAR_MUX_SOCKET))
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($request + "`n")
    $null = $socket.Send($bytes)
    $buffer = New-Object byte[] 4096
    $null = $socket.Receive($buffer)
    $socket.Close()
} catch {
}
exit 0
