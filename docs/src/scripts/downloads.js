// Finds the files of MP Forge's latest release, and which of them suits the visitor's computer.
//
// Nothing here knows what a release's files are called. A file is told apart by its ending alone, so the page
// keeps working whatever the installers are named, and whichever of them a release holds.

const LATEST_RELEASE = 'https://api.github.com/repos/Forge-Audio/mp-forge/releases/latest';

// The endings an installer for each system can have.
const ENDINGS = {
	macos: ['.dmg'],
	windows: ['.msi', '.exe'],
	linux: ['.appimage', '.deb', '.rpm'],
};
export const SYSTEMS = Object.keys(ENDINGS);

const SIGNS_OF_A_SYSTEM = [
	// A phone or a tablet runs none of the three, though its browser may name one of them.
	{ system: null, pattern: /android|iphone|ipad|ipod/i },
	{ system: 'windows', pattern: /win/i },
	{ system: 'macos', pattern: /mac/i },
	{ system: 'linux', pattern: /linux|x11|cros/i },
];

/** The system the visitor's browser says it runs on, or nothing when it is none of the three. */
export function systemOf(browser) {
	const said = `${browser.userAgentData?.platform ?? ''} ${browser.platform ?? ''} ${browser.userAgent ?? ''}`;
	const sign = SIGNS_OF_A_SYSTEM.find((known) => known.pattern.test(said));
	return sign ? sign.system : null;
}

function endingOf(fileName) {
	const lowered = fileName.toLowerCase();
	return Object.values(ENDINGS).flat().find((ending) => lowered.endsWith(ending)) ?? null;
}

function installerFrom(asset) {
	return { name: asset.name, address: asset.browser_download_url, kind: endingOf(asset.name).slice(1) };
}

/** A release's installers, sorted under the system each is for. A system the release has nothing for gets none. */
export function installersBySystem(assets) {
	const installers = assets.filter((asset) => asset?.name && asset.browser_download_url && endingOf(asset.name));
	const forSystem = (system) => installers.filter((asset) => ENDINGS[system].includes(endingOf(asset.name))).map(installerFrom);
	return Object.fromEntries(SYSTEMS.map((system) => [system, forSystem(system)]));
}

function hasAnInstaller(installers) {
	return SYSTEMS.some((system) => installers[system].length > 0);
}

/**
 * Asks GitHub for the latest release. Answers with its version and its installers, or with nothing when there
 * is no release yet, when it holds no installer, or when GitHub could not be asked.
 */
export async function latestRelease(ask = fetch) {
	try {
		const answer = await ask(LATEST_RELEASE, { headers: { accept: 'application/vnd.github+json' } });
		if (!answer.ok) return null;
		const release = await answer.json();
		const installers = installersBySystem(Array.isArray(release.assets) ? release.assets : []);
		return hasAnInstaller(installers) ? { version: release.tag_name ?? '', installers } : null;
	} catch {
		return null;
	}
}
