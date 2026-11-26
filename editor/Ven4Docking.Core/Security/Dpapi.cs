using System.Security.Cryptography;
using System.Text;

namespace Ven4Docking.Core.Security;

/// <summary>
/// Secrets in config.json (spec 1.1 §2): <c>ProtectedData.Protect</c> of the UTF-8 bytes, scope CurrentUser,
/// no extra entropy, base64. The agent runs as the same user and decrypts with <c>CryptUnprotectData</c>.
/// </summary>
public static class Dpapi
{
    public static string Protect(string plain)
    {
        byte[] data = Encoding.UTF8.GetBytes(plain);
        try
        {
            return Convert.ToBase64String(ProtectedData.Protect(data, null, DataProtectionScope.CurrentUser));
        }
        finally { CryptographicOperations.ZeroMemory(data); }
    }

    /// <summary>False for anything that is not our ciphertext (bad base64, another user or PC).</summary>
    public static bool TryUnprotect(string? base64, out string plain)
    {
        plain = "";
        if (string.IsNullOrWhiteSpace(base64)) return false;
        try
        {
            byte[] data = ProtectedData.Unprotect(Convert.FromBase64String(base64.Trim()), null, DataProtectionScope.CurrentUser);
            plain = Encoding.UTF8.GetString(data);
            CryptographicOperations.ZeroMemory(data);
            return true;
        }
        catch (Exception e) when (e is FormatException or CryptographicException) { return false; }
    }
}
