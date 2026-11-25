using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;

namespace Ven4Docking.Core.Net;

/// <summary>Picks the PC's address that a phone on the same LAN can reach (for the wizard's url).</summary>
public static class LanAddress
{
    public static bool IsPrivate(IPAddress ip)
    {
        if (ip.AddressFamily != AddressFamily.InterNetwork) return false;
        var b = ip.GetAddressBytes();
        return b[0] == 10
               || (b[0] == 172 && b[1] >= 16 && b[1] <= 31)
               || (b[0] == 192 && b[1] == 168)
               || (b[0] == 100 && b[1] >= 64 && b[1] <= 127);
    }

    /// <summary>192.168/16 and 10/8 with a default gateway first; Tailscale-like 100.64/10 last.</summary>
    public static IPAddress? GetPreferred()
    {
        var candidates = new List<(IPAddress Ip, int Score)>();
        foreach (var ni in NetworkInterface.GetAllNetworkInterfaces())
        {
            if (ni.OperationalStatus != OperationalStatus.Up || ni.NetworkInterfaceType == NetworkInterfaceType.Loopback) continue;
            IPInterfaceProperties props;
            try { props = ni.GetIPProperties(); } catch (NetworkInformationException) { continue; }
            bool hasGateway = props.GatewayAddresses.Any(g => g.Address.AddressFamily == AddressFamily.InterNetwork && !g.Address.Equals(IPAddress.Any));
            foreach (var ua in props.UnicastAddresses)
            {
                var ip = ua.Address;
                if (!IsPrivate(ip)) continue;
                var b = ip.GetAddressBytes();
                int score = (hasGateway ? 10 : 0) + (b[0] == 192 ? 3 : b[0] == 10 ? 2 : b[0] == 172 ? 1 : -5);
                candidates.Add((ip, score));
            }
        }
        return candidates.OrderByDescending(c => c.Score).Select(c => c.Ip).FirstOrDefault();
    }

    public static string AgentUrl(IPAddress ip, int port) => $"http://{ip}:{port}/";
}
