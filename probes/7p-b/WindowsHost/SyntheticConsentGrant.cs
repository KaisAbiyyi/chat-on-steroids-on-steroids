namespace CoSoS.Probe7PB.WindowsHost;

/// <summary>
/// Host-owned authorization fixture: this never represents a real OAuth token
/// exchange or grants access to a provider. The browser cannot choose the
/// expected callback, profile or outstanding state.
/// </summary>
internal sealed class SyntheticConsentGrant
{
    private const string Callback = "https://synthetic.cos.test/oauth-callback.html";
    private readonly string _profile;
    private readonly string _state;
    private bool _consumed;

    public SyntheticConsentGrant(string profile, string state)
    {
        _profile = profile;
        _state = state;
    }

    public string AuthorizationUrl => "https://login.synthetic.cos.test/oauth-consent.html?redirect_uri="
        + Uri.EscapeDataString(Callback) + "&state=" + Uri.EscapeDataString(_state);

    public bool Complete(string profile, string callbackUrl, bool approved)
    {
        if (_consumed || profile != _profile ||
            !Uri.TryCreate(callbackUrl, UriKind.Absolute, out var uri) ||
            uri.GetLeftPart(UriPartial.Path) != Callback || uri.UserInfo.Length != 0 ||
            uri.Fragment.Length != 0)
            return false;

        var query = System.Web.HttpUtility.ParseQueryString(uri.Query);
        if (query.Count != 2 || query.GetValues("state") is not [var state] || state != _state)
            return false;

        if (approved ? query.GetValues("code") is not ["synthetic-code-only"]
                     : query.GetValues("error") is not ["access_denied"])
            return false;

        _consumed = true;
        return true;
    }
}
