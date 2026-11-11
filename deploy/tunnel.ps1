# Keeps http://localhost:5050 connected to the explorer on the server.
# Reconnects on its own after sleep, Wi-Fi drops or a server reboot.
param(
    [string]$Server = $(if ($env:EXPLORER_SERVER) { $env:EXPLORER_SERVER } else { "user@your-server" }),
    [int]$Port = 5050
)

while ($true) {
    & ssh -N `
        -o BatchMode=yes `
        -o ExitOnForwardFailure=yes `
        -o ServerAliveInterval=30 `
        -o ServerAliveCountMax=3 `
        -L "${Port}:localhost:${Port}" `
        $Server
    Start-Sleep -Seconds 5
}
