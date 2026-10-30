# Google sign-in research (2026-10-30)

## Question

Should the calendar page offer "Sign in with Google" in place of pasting the secret iCal address (`calendar.ics_url`,
see `docs/configuration.md`), with the pasted address kept as a fallback? This doc records what Google and Microsoft
document about the scope, verification, token lifetime, the desktop flow and token storage, what the owner would have
to create and keep up, and what the change would add to `notch.exe`. The spec allows Google sign-in only for reading
the user's calendar and rules out an account system of our own (`docs/superpowers/specs/2026-10-30-settings-window-design.md`,
"Non-goals").

## Sources

Every page below was read on 2026-10-30. Claims cite the source key in brackets. Quotes are from the page as served
that day; where the reading tool returned a paraphrase, the doc paraphrases and does not use quotation marks.

| Key | Page |
| --- | --- |
| S1 | Choose Google Calendar API scopes, https://developers.google.com/workspace/calendar/api/auth |
| S2 | OAuth 2.0 Scopes for Google APIs, https://developers.google.com/identity/protocols/oauth2/scopes |
| S3 | Sensitive scope verification, https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification |
| S4 | Restricted scope verification, https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification |
| S5 | Brand verification, https://developers.google.com/identity/protocols/oauth2/production-readiness/brand-verification |
| S6 | Verification requirements, https://support.google.com/cloud/answer/13464321 |
| S7 | Unverified apps, https://support.google.com/cloud/answer/7454865 |
| S8 | Manage app audience, https://support.google.com/cloud/answer/15549945 |
| S9 | Privacy policy requirements, https://support.google.com/cloud/answer/13806988 |
| S10 | OAuth app verification FAQ, https://support.google.com/cloud/answer/13463817 |
| S11 | Using OAuth 2.0 to Access Google APIs (refresh token expiration), https://developers.google.com/identity/protocols/oauth2 |
| S12 | OAuth 2.0 for Desktop Apps, https://developers.google.com/identity/protocols/oauth2/native-app |
| S13 | Events: list, https://developers.google.com/workspace/calendar/api/v3/reference/events/list |
| S14 | Calendar API usage limits, https://developers.google.com/workspace/calendar/api/guides/quota |
| S15 | Sync Google Calendar with other calendar applications (secret address), https://support.google.com/calendar/answer/37648 |
| S16 | Make your calendar public, https://support.google.com/calendar/answer/37083 |
| S17 | CalDAV API developer's guide, https://developers.google.com/workspace/calendar/caldav/v2/guide |
| S18 | Configure the OAuth consent screen, https://developers.google.com/workspace/guides/configure-oauth-consent |
| S19 | Create access credentials, https://developers.google.com/workspace/guides/create-credentials |
| S20 | Google API Services User Data Policy, https://developers.google.com/terms/api-services-user-data-policy |
| S21 | Search Console: verify your site ownership, https://support.google.com/webmasters/answer/9008080 |
| M1 | CredWriteW function (wincred.h), https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew |
| M2 | CREDENTIALW structure (wincred.h), https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw |
| M3 | HttpClient.PostAsync (Windows.Web.Http), https://learn.microsoft.com/en-us/uwp/api/windows.web.http.httpclient.postasync |
| M4 | windows crate 0.62.2, `CredWriteW`, https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/Credentials/fn.CredWriteW.html |
| R1 | Rust std `TcpListener`, https://doc.rust-lang.org/std/net/struct.TcpListener.html |
| Repo | This repository at commit `9c77d07` plus the `windows-0.62.2` crate source in the local Cargo registry |

## Findings

### 1. Scope and verification

**Scope.** Two read-only scopes read event titles and times, and `events.list` accepts both [S13]:

| Scope string | Google's description [S1] |
| --- | --- |
| `https://www.googleapis.com/auth/calendar.events.owned.readonly` | "See the events on Google calendars you own." |
| `https://www.googleapis.com/auth/calendar.events.readonly` | "View events on all your calendars." |

`calendar.events.owned.readonly` is the narrower one and covers the user's primary calendar. `calendar.events.readonly`
also covers calendars the user subscribes to. The notch reads one calendar today, so the owned scope is enough.
Google requires a justification for why a narrower scope would not work [S6], which favors the owned scope.

**Classification.** Google gives "reading events stored in Google Calendar" as an example of a sensitive scope [S3].
None of S1, S2 or S6 labels an individual Calendar scope string; S2 says the Cloud Console shows a sensitive indicator
on the consent screen configuration. No page I read lists a Calendar scope as restricted. I cannot confirm the exact
label of `calendar.events.owned.readonly` from a documentation page; the Cloud Console shows it when the scope is added.

**What a public app needs.** For an External app in production that requests a sensitive scope:

- Brand verification first: "you must have a published branding status before you can request verification for data
  access" [S3]. Brand verification checks a publicly accessible homepage, a privacy policy "hosted within the same
  domain as your application's home page", authorized domains listed as top private domains, and domain ownership
  verified in Google Search Console [S5]. S6 adds: "The homepage must be hosted on a verified domain you own."
- Sensitive-scope verification: a detailed justification per scope, a demonstration video uploaded to YouTube as
  Unlisted that shows the English consent screen, the app name, the OAuth client ID in the address bar and each scope's
  feature [S3]; the Limited Use rules on data use [S6, S20].
- Security assessment: S6 ties the annual assessment by a Google-empanelled assessor to restricted scopes only, and S4
  repeats it for restricted data reachable through a third-party server. S20 says applications using "Sensitive" and
  "Restricted" scopes must pass an annual assessment depending on the API and the number of user grants or users. I
  cannot confirm that a sensitive Calendar scope never triggers an assessment; S20 gives no threshold.
- Lead time stated by Google: brand verification "usually completes in a few minutes" when automated and takes 2-3
  business days in manual review [S5]; sensitive-scope verification "can take up to 10 days" [S3], listed as
  "10 Business days" in S10, which adds "these estimates are not guaranteed." S10 says Google charges nothing for a
  security assessment and that an assessor's fee is agreed between developer and assessor. No page I read states a fee
  for brand or sensitive-scope verification.

**Unverified app.** An app that requests a sensitive scope without verification "might display an "unverified app"
screen before it displays the consent screen" [S7], and the quota is "100 new users in total, after the app presents
the unverified app screen" [S7]. That cap "applies over the entire lifetime of the project, and it cannot be reset or
changed" [S10]. S7 does not describe how a user gets past the screen. Verification is not required when "you are the
only user of your app or if your app is used by only a few users, all of whom are known personally to you" [S3].

**Testing status.** An app in Testing is "limited to up to 100 test users listed in the OAuth consent screen", and
Google shows test users a warning before they authorize [S8]. Verification is not required while the app stays in
Testing [S3].

### 2. Refresh-token lifetime

- A project with External user type in Testing status gets refresh tokens that expire after 7 days, unless it asks
  only for name, email and profile scopes [S11]. S8 says test-user authorizations expire seven days after consent.
- In production, a refresh token stops working when the user revokes access, when it goes unused for six months, when
  the account exceeds its live-token cap, when time-based access ends, or when an admin restricts the service [S11].
  The password-change rule applies only to Gmail scopes [S11].
- Google keeps at most 100 refresh tokens per Google Account per OAuth client ID and invalidates the oldest one without
  warning past that [S11].
- The app revokes a token with `https://oauth2.googleapis.com/revoke`; revoking an access token also revokes its
  refresh token [S12].

### 3. Desktop flow

- Client type: **Desktop app**, created under Google Auth platform > Clients [S12, S19]. A Desktop client has no
  redirect URI field [S19].
- Redirect: the loopback address is "the recommended mechanism"; the app starts an HTTP listener on a random free port
  and uses `http://127.0.0.1:port` or `http://[::1]:port`. `localhost` works but "may cause issues with client
  firewalls" [S12].
- "Custom URI schemes are no longer supported due to the risk of app impersonation", and the manual copy-paste (OOB)
  flow is no longer supported [S12].
- PKCE: `code_challenge_method` is `S256` (recommended) or `plain`, and Google says to always use PKCE [S12].
- Client secret: "Desktop apps are public clients and cannot keep secrets confidential", and `client_secret` is listed
  as optional in the code exchange and refresh requests [S12]. The client ID ships inside `notch.exe` and the public
  source either way.
- Refresh token: "refresh tokens are always returned for installed applications" [S12].

Requests the app makes [S12, S13]:

| Step | Request | Parameters |
| --- | --- | --- |
| Authorize (in the browser) | `GET https://accounts.google.com/o/oauth2/v2/auth` | `client_id`, `redirect_uri=http://127.0.0.1:<port>`, `response_type=code`, `scope`, `code_challenge`, `code_challenge_method=S256`, `state`; optional `login_hint` |
| Receive the code | Google redirects the browser to the loopback listener | query `code`, `state` (the app checks `state`) |
| Exchange the code | `POST https://oauth2.googleapis.com/token` (form body) | `client_id`, `client_secret` (optional), `code`, `code_verifier`, `grant_type=authorization_code`, `redirect_uri` |
| Refresh | `POST https://oauth2.googleapis.com/token` | `client_id`, `client_secret` (optional), `grant_type=refresh_token`, `refresh_token` |
| Read events | `GET https://www.googleapis.com/calendar/v3/calendars/primary/events` with a Bearer access token | `timeMin`, `timeMax` (RFC 3339 with offset), `singleEvents=true`, `orderBy=startTime`, `maxResults` (default 250, at most 2500), `pageToken` |
| Sign out | `POST https://oauth2.googleapis.com/revoke` | `token` |

### 4. Alternatives and the API surface

| Way in | What the user does | Privacy tradeoff |
| --- | --- | --- |
| Secret iCal address (current) | Settings > the calendar > Integrate calendar > copy "Secret address in iCal format" [S15] | Anyone with the address reads the calendar; Google says "Only you should know the Secret Address" and offers Reset to replace it [S15]. A work or school admin may have disabled it [S15]. The notch stores it in `settings.txt` today. |
| Public address | Tick "Make available to public", then copy "Public address in iCal format"; "The iCal address only works if the calendar is public" [S16] | Every event is public (or only free/busy with "See only free/busy (hide details)", which hides titles) [S16]. Not a fit for a personal calendar. |
| CalDAV | Nothing simpler: CalDAV needs HTTPS and OAuth 2.0, a Cloud project, the CalDAV API enabled and a client ID; Basic authentication returns `401` [S17] | Same verification path as the Calendar API, so it saves nothing. |

`events.list` is the only Calendar API call the page needs [S13]. `syncToken` cannot be combined with `timeMin`,
`timeMax` or `orderBy`, and an expired token returns `410 GONE` [S13]; the notch shows a short time window, so a plain
`timeMin`/`timeMax` query each refresh is the simpler fit.

Quota [S14]: 10,000 requests per minute per project, 600 per minute per user per project, and 1,000,000 per day per
project. "All standard use of the Google Calendar API is available at no additional cost", and "Exceeding the quota
request limits is planned to incur charges to your Google Cloud billing account later in 2026", with "at least 90 days'
notice". Calculation: one request every 15 minutes (the current `calendar.refresh_minutes` default) is 24 × 60 / 15 =
96 requests per user per day, so the daily project limit covers 1,000,000 / 96 ≈ 10,416 users before paging or retries.
S14 calls polling every calendar an anti-pattern and recommends push notifications; push needs a public HTTPS receiver,
which a server-less desktop app lacks (speculation: S14 does not say this, it follows from push delivering to a URL).

### 5. What the owner creates

| Item | Detail | Source |
| --- | --- | --- |
| Cloud project | one project; the Calendar API enabled in it | S19 (links "Enable Google Workspace APIs") |
| Consent screen: Branding | app name, user support email; for brand verification also logo, homepage, privacy policy link, authorized domains | S18, S5 |
| Consent screen: Audience | user type External; test users while in Testing | S18, S8 |
| Consent screen: Contact | email for project notices | S18 |
| Consent screen: Data access | add `calendar.events.owned.readonly` | S18 |
| User Data Policy | accept it during setup | S18, S20 |
| OAuth client | type Desktop app; no redirect URI to register | S19 |
| Homepage and privacy policy | homepage "hosted on a verified domain you own" [S6]; policy on the homepage's domain [S5]; the policy must be HTML in "the body of a dedicated privacy policy web page" and "Hosted on a domain that you own" [S9] | S5, S6, S9 |
| Domain ownership | verify each authorized domain in Search Console; a Domain property needs a DNS record, a URL-prefix property also accepts an HTML file or meta tag [S21] | S5, S21 |
| Demo video and justification | for sensitive-scope verification | S3 |

**Does a README privacy section on GitHub satisfy the requirement?** No for `github.com/hibatillah/notch`: S6 requires
the homepage on "a verified domain you own" and S9 requires the policy "Hosted on a domain that you own" on a dedicated
privacy policy page; the owner does not own `github.com` and a README is not a dedicated policy page. For a GitHub
Pages site (`hibatillah.github.io`): S21 allows HTML-file verification of a URL-prefix property, but no Google page I
read says whether a `github.io` subdomain is accepted as an authorized domain, so I cannot confirm this. A custom domain
the owner registers meets the text of S5, S6 and S9; its price is not a Google figure, so I cannot confirm it here.

### 6. Token storage and the README

**Storage.** Windows Credential Manager through `CredWriteW` (Advapi32.dll, `wincred.h`), which "creates a new
credential or modifies an existing credential in the user's credential set" and replaces an existing one with the same
`TargetName` and `Type` [M1]. Use `Type = CRED_TYPE_GENERIC` ("stored securely but has no other significant
characteristics"), a `TargetName` prefixed with the program name such as `notch/google-calendar`, and
`Persist = CRED_PERSIST_LOCAL_MACHINE`, which keeps it on this computer for later logons and out of roaming [M2].
`CredentialBlobSize` is at most `CRED_MAX_CREDENTIAL_BLOB_SIZE` (5 × 512 = 2,560 bytes) [M2]; a refresh token fits (its
length is not documented by Google in the pages I read, so I cannot confirm the margin). The `windows` crate exposes
`CredWriteW(credential: *const CREDENTIALW, flags: u32) -> Result<()>` in `windows::Win32::Security::Credentials` [M4];
`CredReadW`, `CredDeleteW` and `CredFree` are in the same module, behind the `Win32_Security_Credentials` feature
[Repo: `windows-0.62.2` source and `Cargo.toml`]. The same store would also suit the pasted iCal address, which is a
secret in `settings.txt` today.

**README edits** (the README today says the notch connects out "for two things only", lyrics and a calendar link):

1. "Device requirements": "Internet only for lyrics and a calendar link" also names Google sign-in.
2. "Updating": "the only things it connects to the internet for are lyrics and a calendar link" also names Google.
3. "Privacy" opening: "no account" stays true for Notch; add that signing in with Google is optional and is the user's
   own Google account.
4. "Privacy" calendar bullet: name the hosts contacted (`accounts.google.com` in the browser, `oauth2.googleapis.com`,
   `www.googleapis.com`), the read-only scope, that only the event list of the primary calendar for the shown time
   window is downloaded every 15 minutes, that nothing is sent anywhere else, that the refresh token sits in Windows
   Credential Manager and not in `settings.txt`, that sign-in briefly listens on `127.0.0.1`, and how to sign out
   (the app revokes the token [S12]).
5. S20 requires the policy to "thoroughly disclose the manner in which your application accesses, uses, stores, or
   shares Google user data" and asks for the notice to be prominent in the app; the settings window's calendar section
   would link to it.
6. `docs/configuration.md` gains the new setting; the README table follows it (repo rule `docs.md`).

### 7. What it adds to `notch.exe`

Verified against the repository:

- HTTPS: `src/pages/calendar/fetch.rs` downloads with WinRT `Windows::Web::Http::HttpClient` (`GetStringAsync`) on a
  background thread and wakes the page with a `Notifier`. `HttpClient::PostAsync(Uri, IHttpContent)` exists [M3], and
  `HttpFormUrlEncodedContent` is in the same `Web_Http` module [Repo]. `Web_Http` and `Web_Http_Headers` are enabled in
  `Cargo.toml`; `HttpCredentialsHeaderValue` for the Bearer header is in `Web_Http_Headers` [Repo].
- JSON: `Data_Json` is enabled, and `src/pages/media/lyrics/client.rs` already parses with `JsonObject::Parse`.
- Loopback listener: `std::net::TcpListener` suffices; "Binding with a port number of 0 will request that the OS
  assigns a port", read back with `local_addr` [R1]. A blocking `accept` on a background thread fits the performance
  rule (wait on a blocking wait); it needs a time limit so an abandoned sign-in does not leave the thread waiting.
- Opening the browser: `ShellExecuteW` is in `Win32_UI_Shell`, already enabled [Repo].
- PKCE needs random bytes and SHA-256: `BCryptGenRandom` and `BCryptHash` are in the `windows` crate behind
  `Win32_Security_Cryptography`, which is not enabled today [Repo].
- Token storage: `Win32_Security_Credentials`, not enabled today [Repo].

No new crate is needed. The change adds two `windows` crate features (`Win32_Security_Credentials`,
`Win32_Security_Cryptography`). Size and memory effect: I cannot confirm this without a build and a `scripts/perf.sh`
run.

## What the owner must do

| Task | Option | Effort | Recurring cost or time |
| --- | --- | --- | --- |
| Create the Cloud project, enable the Calendar API, fill the consent screen, create a Desktop client | A, B | under an hour (estimate, not a Google figure) | none stated by Google |
| Register a domain and host a homepage and privacy policy on it | A | a small static site | domain renewal; price: I cannot confirm this |
| Verify the domain in Search Console | A | one DNS record or HTML file [S21] | keep it verified |
| Brand verification | A | submit in the console | minutes automated, 2-3 business days manual [S5] |
| Sensitive-scope verification: justification and demo video | A | record and upload a video | up to 10 business days, not guaranteed [S3, S10]; resubmit when scopes change |
| Security assessment | A, if Google requires one for this scope | assessor engagement | annual; fee set by the assessor [S10]; whether it applies: I cannot confirm this |
| Watch quota and the planned 2026 charges | A | read Google's notices | 90 days' notice promised [S14] |
| Add himself as a test user, sign in again every 7 days | B (Testing) | minutes | weekly re-sign-in [S11] |
| Rewrite the iCal setup steps in the settings window and README | C | a doc and copy change | none |

## Options

**A. Google sign-in with a verified app.** Any Google user signs in with no warning screen. The owner needs an owned
domain, a homepage and policy on it, brand and sensitive-scope verification, a demo video, and possibly an annual
assessment, and he answers Google's notices for as long as the client exists. The client ID is public in the source;
forks that keep it count against the owner's quota and user cap (speculation: no Google page addresses forks).

**B. Google sign-in left unverified, for the owner only.** In Testing the owner adds himself as a test user and signs
in again every 7 days [S8, S11]. Publishing to production unverified instead keeps tokens alive until six months of
non-use [S11], shows the unverified-app screen, and allows 100 new users over the project's lifetime [S7, S10]; the
personal-use exemption covers only the owner and people he knows [S3]. Released builds would show every other user
the warning, or block them, so B serves the owner's own testing and not the public.

**C. Keep the pasted iCal address and improve its setup.** No Google project, no domain, no verification, no token
expiry, and no new code beyond copy. The settings window can show Google's four steps from S15 next to the field, link
the Reset step, and the address can move from `settings.txt` to Credential Manager with the same API as finding 6. The
privacy cost stays: the address grants read access to the whole calendar to anyone who obtains it [S15].

**D. Do nothing.** The tray menu keeps setting the address from the clipboard. Users keep finding the address without
guidance in the new settings window.

## Recommendation

C. The spec limits Google sign-in to reading one calendar, and the secret address already reads it with no account,
no server and no obligations toward Google. A turns a read-only convenience into a standing compliance task for a solo
owner: an owned domain and site the project does not otherwise need, a review measured in business days with no
guarantee, a possible annual assessment Google's two pages describe differently, and quota charges Google plans for
later in 2026. B cannot ship to users. D leaves the new settings window without the setup help the calendar field
needs. If the owner wants sign-in later, C costs nothing to keep as the fallback the brief already requires, and A can
follow once a domain exists.

## Decision needed from the owner

1. Pick C, A, or B-for-yourself-first.
2. For A: are you willing to register and keep a domain, host a homepage and privacy policy on it, and record the demo
   video?
3. For A: accept `calendar.events.owned.readonly` (primary calendar only), or do you need subscribed calendars
   (`calendar.events.readonly`)?
4. For C: should the pasted address move from `settings.txt` into Windows Credential Manager?
