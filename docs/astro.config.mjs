// @ts-check
import { fileURLToPath } from 'node:url';

import { defineConfig } from 'astro/config';
import { satteri } from '@astrojs/markdown-satteri';
import starlight from '@astrojs/starlight';
import starlightLinksValidator from 'starlight-links-validator';

import { linksBetweenPages } from './src/markdown/links-between-pages.mjs';
import { DISCORD, SOURCE } from './src/outside-places.js';

// The site is published by GitHub Pages as a project site, so every address on it starts with the project's name.
const SITE = 'https://forge-audio.github.io';
const BASE = '/mp-forge';
const PAGES_FOLDER = fileURLToPath(new URL('./src/content/docs', import.meta.url));
// The website's home and its download page, which live in src/pages beside the docs.
const WEBSITE_PAGES = [`${BASE}/`, `${BASE}/download/`];

// The app's own fonts, bundled with the site so that a reader's browser asks nobody else for them.
const APP_FONTS = [
	'@fontsource/pixelify-sans/400.css',
	'@fontsource/pixelify-sans/500.css',
	'@fontsource/pixelify-sans/600.css',
	'@fontsource/pixelify-sans/700.css',
	'@fontsource/silkscreen/400.css',
	'@fontsource/silkscreen/700.css',
];

export default defineConfig({
	site: SITE,
	base: BASE,
	markdown: {
		processor: satteri({ mdastPlugins: [linksBetweenPages({ base: BASE, pagesFolder: PAGES_FOLDER })] }),
	},
	integrations: [
		starlight({
			title: 'MP Forge',
			description: 'Make plugins for your Akai MPC creatively. Free and open source.',
			logo: { src: './src/assets/logo.png', alt: '' },
			favicon: '/favicon.png',
			head: [{ tag: 'link', attrs: { rel: 'icon', href: `${BASE}/favicon.ico`, sizes: '32x32' } }],
			social: [
				{ icon: 'discord', label: 'Forge Audio on Discord', href: DISCORD },
				{ icon: 'github', label: 'MP Forge on GitHub', href: SOURCE },
			],
			customCss: [...APP_FONTS, './src/styles/app-colours.css', './src/styles/app-look.css'],
			sidebar: [
				{ label: 'Download MP Forge', link: '/download/' },
				{ label: 'Start here', items: ['getting-started', 'what-you-need'] },
				{ label: 'Making plugins', items: ['choosing-an-ai', 'first-plugin', 'safety-checks', 'send-to-your-mpc', 'sharing-plugins', 'online-plugins'] },
				{ label: 'When something is wrong', items: ['troubleshooting', 'status'] },
				{ label: 'For developers', items: ['building-from-source'] },
			],
			// The build stops when a link in the docs leads nowhere. The website's own pages are not docs pages, so
			// the links from the docs to them are named here and left to be looked at by hand.
			plugins: [starlightLinksValidator({ exclude: WEBSITE_PAGES })],
		}),
	],
});
