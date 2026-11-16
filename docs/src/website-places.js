// Where the website's pages link to. Every address on the site starts with the site's base, so it is put in
// front here, once.

const BASE = import.meta.env.BASE_URL.replace(/\/$/, '');
const SOURCE = 'https://github.com/Forge-Audio/mp-forge';

const onTheSite = (path) => `${BASE}${path}`;

export const places = {
	home: onTheSite('/'),
	download: onTheSite('/download/'),
	docs: onTheSite('/getting-started/'),
	whatYouNeed: onTheSite('/what-you-need/'),
	choosingAnAi: onTheSite('/choosing-an-ai/'),
	firstPlugin: onTheSite('/first-plugin/'),
	safetyChecks: onTheSite('/safety-checks/'),
	sendToYourMpc: onTheSite('/send-to-your-mpc/'),
	status: onTheSite('/status/'),
	buildingFromSource: onTheSite('/building-from-source/'),
	favicon: onTheSite('/favicon.png'),
	sitemap: onTheSite('/sitemap-index.xml'),
	source: SOURCE,
	releases: `${SOURCE}/releases/latest`,
	licence: `${SOURCE}/blob/main/LICENSE`,
};
