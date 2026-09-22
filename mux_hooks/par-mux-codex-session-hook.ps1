# par-mux codex session hook (Windows) — reports session identity to the
# par-mux daemon on codex SessionStart events. PowerShell port of the POSIX
# reporter (par-mux-codex-session-hook.sh): one JSON line over the control
# socket, one reply read, close. Inert outside a par-mux pane (env guards)
# and silent on every failure.
# PAR_MUX_INTEGRATION_ID=codex
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

# codex names its events claude-style; only session starts are ours to report.
if ($null -eq $payload) { exit 0 }
if ($payload.hook_event_name -isnot [string] -or $payload.hook_event_name -cne "SessionStart") { exit 0 }

$sessionId = $payload.session_id
if ([string]::IsNullOrWhiteSpace($sessionId)) { exit 0 }

# codex exports the pane's thread id into hook processes. When a fork or
# subagent fires SessionStart under a different id, that session is not this
# pane's session — do not overwrite the roster with it.
$inherited = $env:CODEX_THREAD_ID
if (-not [string]::IsNullOrWhiteSpace($inherited) -and $inherited -ne $sessionId) { exit 0 }

$source = $payload.source
if ([string]::IsNullOrWhiteSpace($source)) { $source = "startup" }

$seq = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() * 1000000
$requestId = "par-mux:codex:$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()):$('{0:D6}' -f (Get-Random -Minimum 0 -Maximum 999999))"
$params = [ordered]@{
    pane_id = "$($env:PAR_MUX_PANE_ID)"
    source = "par-mux:codex"
    agent = "codex"
    seq = $seq
    session_start_source = "$source"
    agent_session_id = "$sessionId"
    session_resume_argv = @("codex", "resume", "$sessionId")
}
if ($payload.transcript_path -is [string] -and -not [string]::IsNullOrWhiteSpace($payload.transcript_path)) {
    $params["agent_session_path"] = "$($payload.transcript_path)"
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
