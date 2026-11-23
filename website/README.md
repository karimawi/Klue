# klue.wkar.im

The Klue website: plain HTML, CSS and a little JavaScript, no build step. Preview it with any static server:

```sh
cd website && python3 -m http.server 8000
```

The download buttons read the newest release from GitHub's API (the repository has to be public for that), and fall back to the Releases page.

## Deploying on Vercel

1. In Vercel, **Add New → Project** and import the Klue repository.
2. Set **Root Directory** to `website`, **Framework Preset** to *Other*, and leave the build and output settings empty. Deploy.
3. In **Settings → Domains**, add `klue.wkar.im`, then add the CNAME record Vercel shows (`klue` → `cname.vercel-dns.com`) at wkar.im's DNS provider. HTTPS is set up automatically.

Every push to the production branch redeploys the site.
