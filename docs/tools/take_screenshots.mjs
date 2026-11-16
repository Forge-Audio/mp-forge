// Takes the screenshots the docs show, from the app's real screen.
//
// Opened outside the app, the screen in app/ui answers its own commands with made-up plugins. So a copy of
// that folder, served as plain files, is the real interface with stand-in data. This script makes the copy,
// changes a little of the stand-in data in the copy, drives the screen to each state and takes a picture.
// Nothing in app/ui is changed, and nothing of anybody's own plugins is read.
//
// Run it from the docs folder:  npm run screenshots

import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const APP_SCREEN = fileURLToPath(new URL('../../app/ui', import.meta.url));
const SCREENSHOTS = fileURLToPath(new URL('../src/assets/screenshots', import.meta.url));
const STAND_IN_DATA = 'backend.js';
// The knobs, faders and switches Akai's SDK gives every plugin. They come with the SDK once it is fetched.
const AKAIS_CONTROL_KIT = fileURLToPath(new URL('../../vendor/mpc-plugin-builder-sdk/ui-kit/mpc-red', import.meta.url));
// Where the app's screen looks for a plugin's control pictures when it is opened outside the app.
const CONTROL_PICTURES_IN_THE_COPY = 'demo-skin';

// A window small enough that its words can still be read once a picture is shrunk to the width of a page, and
// large enough that nothing in the plugin's view is cut short. It is drawn at twice the detail so the pictures
// stay sharp.
const WINDOW = { width: 1120, height: 800 };
const DETAIL = 2;
const ROOM_ROUND_A_PART = 8;
const OUT_OF_THE_WAY = { x: 1, y: 1 };
// Dates and times are written the same way wherever the pictures are taken.
const SAME_ON_EVERY_COMPUTER = { colorScheme: 'dark', locale: 'en-GB', timezoneId: 'UTC' };

const KINDS_OF_FILE = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png' };

const MAKER = 'Forge Audio';
const EFFECT = { name: 'Dusty Tape', words: 'A worn tape echo that gets seasick when I turn it up.' };
const INSTRUMENT = 'Glass Pad';
const MPC_ADDRESS = '192.168.1.50';
const STATUS_WHEN_SAFE = 'SAFE TO SEND';
// Counted from nothing along the timeline of versions: the newest of the earlier ones.
const EARLIER_VERSION_SHOWN = 2;

// The checks the app really runs, in its order, so the pictures say what the pages say.
const THE_CHECKS = [
	'Code stays inside the plugin', 'Pictures can be drawn', 'Builds for every MPC', 'Screen loads with its pictures',
	'Moving pictures move', 'Survives everyday use', 'Survives a full chord', 'Survives a playing song',
	'Survives the smallest buffer', 'Survives the largest buffer', 'Survives a 48 kHz project', 'Survives a long session',
];

// What is changed in the copy's stand-in data, each as [what is there, what goes in its place].
const CHANGES_FOR_EVERY_PICTURE = [
	// The stand-in starts without Akai's build tools and fetches them as the app would. The pictures are of a
	// computer that has them already, so none shows the note about getting ready.
	[
		'{ name: "Akai\'s build tools", usedFor: "The compiler that turns a plugin\'s code into what the MPC runs.", isThere: false',
		'{ name: "Akai\'s build tools", usedFor: "The compiler that turns a plugin\'s code into what the MPC runs.", isThere: true',
	],
	// The only maker shown is the project itself.
	[/maker: "Demo"/g, `maker: "${MAKER}"`],
	// The made-up plugins wear the control kit that comes with Akai's SDK, which is the one copied in for them.
	[/kit: "(cyan|amber)"/g, 'kit: "mpc-red"'],
	// The plugin the pictures follow is made in front of the camera, so it is not there to begin with.
	['const plugins = [{ name: "Dusty Tape", maker: "Forge Audio", kind: "effect" }, ', 'const plugins = ['],
	[
		'checks: ["Code stays inside the plugin", "Builds for every MPC", "Survives everyday use", "Survives a full chord"]',
		`checks: ${JSON.stringify(THE_CHECKS)}`,
	],
	// The stand-in's closing words say they are a demo. The example the app's own guide shows says what a build says.
	[
		'"This is the demo, so nothing was really built.\\n\\n**Controls:** Time, Repeats and Tone."',
		'"**Built.** Time and Repeats set the echo, Wobble and Flutter make it seasick, and Dirt and Hiss wear the tape. It passed its safety check."',
	],
];

function changed(text, [from, to]) {
	const result = text.replace(from, to);
	if (result === text) throw new Error(`The app's stand-in data no longer holds ${from}. Bring this script up to date with app/ui/${STAND_IN_DATA}.`);
	return result;
}

// --- The copy of the app's screen, and serving it -------------------------------------------------

async function copyOfTheAppScreen() {
	const copy = await mkdtemp(path.join(os.tmpdir(), 'mp-forge-screen-'));
	await cp(APP_SCREEN, copy, { recursive: true });
	await cp(AKAIS_CONTROL_KIT, path.join(copy, CONTROL_PICTURES_IN_THE_COPY), { recursive: true }).catch(() => {
		throw new Error("Akai's SDK is not in vendor/ yet, and the pictures need its control kit. Fetch it first: see Building from source.");
	});
	const standInData = path.join(copy, STAND_IN_DATA);
	const original = await readFile(standInData, 'utf8');
	await writeFile(standInData, CHANGES_FOR_EVERY_PICTURE.reduce(changed, original));
	return copy;
}

function serving(folder) {
	const server = http.createServer(async (request, response) => {
		const asked = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
		const file = path.join(folder, path.normalize(asked === '/' ? '/index.html' : asked));
		try {
			const contents = await readFile(file);
			response.writeHead(200, { 'content-type': KINDS_OF_FILE[path.extname(file)] ?? 'application/octet-stream' });
			response.end(contents);
		} catch {
			response.writeHead(404).end();
		}
	});
	return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

// --- Opening the screen ----------------------------------------------------------------------------

// A fresh window on the app's screen, past the tour it offers the first time, with every font loaded.
async function openedScreen(browser, address) {
	const context = await browser.newContext({ viewport: WINDOW, deviceScaleFactor: DETAIL, ...SAME_ON_EVERY_COMPUTER });
	await context.addInitScript(() => localStorage.setItem('guideSeen', 'yes'));
	const page = await context.newPage();
	await page.goto(address);
	await page.evaluate(() => document.fonts.ready);
	return page;
}

// Lets what is moving come to rest, with the pointer where it brings up no name for what is under it.
async function settled(page) {
	await page.mouse.move(OUT_OF_THE_WAY.x, OUT_OF_THE_WAY.y);
	await page.waitForTimeout(700);
}

async function bringIntoTheMiddle(page, selector) {
	await page.locator(selector).evaluate((part) => part.scrollIntoView({ block: 'center' }));
}

// --- Taking the pictures ---------------------------------------------------------------------------

async function pictureOfTheWindow(page, name) {
	await settled(page);
	await page.screenshot({ path: path.join(SCREENSHOTS, `${name}.png`) });
	console.log(`took ${name}.png`);
}

// A picture of the band of the window that some parts stand in, from one side of the window to the other. Every
// picture is as wide as the window, so they are all shown at the same size on a page.
async function pictureOfTheBandWith(page, name, selectors) {
	await settled(page);
	const boxes = await Promise.all(selectors.map((selector) => page.locator(selector).first().boundingBox()));
	const top = Math.max(0, Math.min(...boxes.map((box) => box.y)) - ROOM_ROUND_A_PART);
	const bottom = Math.min(WINDOW.height, Math.max(...boxes.map((box) => box.y + box.height)) + ROOM_ROUND_A_PART);
	await page.screenshot({ path: path.join(SCREENSHOTS, `${name}.png`), clip: { x: 0, y: top, width: WINDOW.width, height: bottom - top } });
	console.log(`took ${name}.png`);
}

const SHEET = '#app-sheet .sheet';

async function showPlugin(page, name) {
	await page.locator('#plugin-tabs .tab', { hasText: name }).click();
	await page.waitForFunction((shown) => document.getElementById('plugin-title').textContent === shown, name);
}

async function waitUntilSafe(page) {
	await page.waitForFunction((safe) => document.getElementById('status').textContent === safe, STATUS_WHEN_SAFE, { timeout: 20000 });
}

async function closeSheet(page) {
	await page.keyboard.press('Escape');
	await page.locator(SHEET).waitFor({ state: 'detached' });
}

async function describeAPlugin(page) {
	await page.fill('#plugin-name', EFFECT.name);
	await page.fill('#maker-name', MAKER);
	await page.fill('#message', EFFECT.words);
	await pictureOfTheWindow(page, 'describe-a-plugin');
}

async function buildIt(page) {
	await page.click('#send');
	await waitUntilSafe(page);
	await pictureOfTheWindow(page, 'the-mpc-screen');
}

async function chooseAnAi(page) {
	await page.click('#ai-menu-button');
	await pictureOfTheBandWith(page, 'choose-an-ai', ['header.bar', '#ai-menu']);
	await page.keyboard.press('Escape');
}

async function goBackAVersion(page) {
	await page.click('#go-back');
	await page.locator('.timeline-stop').nth(EARLIER_VERSION_SHOWN).click();
	await pictureOfTheWindow(page, 'versions');
	await closeSheet(page);
}

async function sendToTheMpc(page) {
	await page.click('#send-to-mpc');
	await page.locator(`${SHEET} input[type="text"]`).fill(MPC_ADDRESS);
	await pictureOfTheWindow(page, 'send-to-your-mpc');
	await closeSheet(page);
}

async function editTheLook(page) {
	await page.click('#edit-look');
	await pictureOfTheWindow(page, 'edit-the-look');
	await page.click('#editor-done');
}

// The instrument comes with the stand-in data and has not been checked, so it shows the check being offered.
async function checkAndPlayTheInstrument(page) {
	await showPlugin(page, INSTRUMENT);
	await page.click('#status-button');
	await pictureOfTheBandWith(page, 'run-the-safety-check', ['header.bar', '#status-menu']);
	await page.click('#check');
	await waitUntilSafe(page);
	await pictureOfTheBandWith(page, 'safety-checks-passed', ['header.bar', '.mirror-heading']);
	await page.click('#play');
	await page.locator('#piano').waitFor({ state: 'visible' });
	await pictureOfTheWindow(page, 'play-an-instrument');
	await page.click('#play');
}

// One setting, found by something it holds.
async function pictureOfSetting(page, name, holds) {
	const setting = `.setting:has(${holds})`;
	await bringIntoTheMiddle(page, setting);
	await pictureOfTheBandWith(page, name, [setting]);
}

async function settingsPictures(page) {
	await page.click('#settings-button');
	await pictureOfSetting(page, 'settings-ai-accounts', '#ai-accounts');
	await pictureOfSetting(page, 'settings-your-mpc', '#mpc-address');
	await page.click('#settings-done');
}

async function whatMpForgeNeeds(page) {
	await page.click('#settings-button');
	await pictureOfSetting(page, 'settings-what-mp-forge-needs', '#needs-list');
}

async function takeEveryPicture(browser, address) {
	const page = await openedScreen(browser, address);
	await describeAPlugin(page);
	await buildIt(page);
	await chooseAnAi(page);
	await goBackAVersion(page);
	await sendToTheMpc(page);
	await editTheLook(page);
	await checkAndPlayTheInstrument(page);
	await settingsPictures(page);
	await whatMpForgeNeeds(await openedScreen(browser, address));
}

async function main() {
	await mkdir(SCREENSHOTS, { recursive: true });
	const copy = await copyOfTheAppScreen();
	const server = await serving(copy);
	const browser = await chromium.launch();
	try {
		await takeEveryPicture(browser, `http://127.0.0.1:${server.address().port}/`);
	} finally {
		await browser.close();
		server.close();
		await rm(copy, { recursive: true, force: true });
	}
}

await main();
