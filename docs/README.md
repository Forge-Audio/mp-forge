# The MP Forge website and docs

One Astro project holds two things, published together:

- **The website**: its home and its download page, in `src/pages/`, with their frame in `src/layouts/` and their
  look in `src/styles/website.css`. The download page's own logic is in `src/scripts/downloads.js`.
- **The docs**: plain Markdown in `src/content/docs/`, shown by Astro Starlight and styled to look like the app
  in `src/styles/app-look.css`.

Both draw with the app's colours, which are written down once, in `src/styles/app-colours.css`.

The home page's first screen shows the launch video. The name YouTube gives the video is written in
`src/launch-video.js`. With no name there, a picture of the app stands in its place.

```sh
npm install        # once
npm run dev        # the site at http://localhost:4321/mp-forge/, redrawn as you edit
npm run build      # builds it into dist/ and stops on a link that leads nowhere
npm run preview    # serves what was built, at the same address
```

A page links to another page by its file, such as `[the safety checks](./safety-checks.md)`. That works on
GitHub, and the build turns it into the page's address on the site.

The pictures in `public/images/` are the ones the README at the top of the repository shows. They are drawn by
`tools/make_readme_art.py`. To draw them again: `python3 tools/make_readme_art.py public/images`

The screenshots in `src/assets/screenshots/` are taken from the app's real screen, shown with made-up plugins.
To take them again after the app's look changes: `npx playwright install chromium` once, then
`npm run screenshots`. Akai's SDK has to be in `vendor/` first, because the pictures use its control kit.

The site is published by `.github/workflows/docs.yml` once the repository is public.
