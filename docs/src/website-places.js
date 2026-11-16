// Where the website's pages link to. Every address on the site starts with the site's base, so it is put in
// front here, once.

import { DISCORD, DONATIONS, SOURCE } from './outside-places.js';

const BASE = import.meta.env.BASE_URL.replace(/\/$/, '');

const onTheSite = (path) => `${BASE}${path}`;

export const places = {
	home: onTheSite('/'),
	download: onTheSite('/download/'),
	docs: onTheSite('/getting-started/'),
	firstOpening: onTheSite('/getting-started/#the-first-time-you-open-it'),
	whatYouNeed: onTheSite('/what-you-need/'),
	choosingAnAi: onTheSite('/choosing-an-ai/'),
	firstPlugin: onTheSite('/first-plugin/'),
	editingTheLook: onTheSite('/first-plugin/#edit-the-look-by-hand'),
	safetyChecks: onTheSite('/safety-checks/'),
	sendToYourMpc: onTheSite('/send-to-your-mpc/'),
	status: onTheSite('/status/'),
	buildingFromSource: onTheSite('/building-from-source/'),
	favicon: onTheSite('/favicon.png'),
	sitemap: onTheSite('/sitemap-index.xml'),
	source: SOURCE,
	discord: DISCORD,
	donations: DONATIONS,
	releases: `${SOURCE}/releases/latest`,
	licence: `${SOURCE}/blob/main/LICENSE`,
};
