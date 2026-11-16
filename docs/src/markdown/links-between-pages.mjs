// Lets one page link to another by its file, as in [the safety checks](./safety-checks.md).
//
// A link written that way works where the pages are read as plain files, on GitHub for one. On the built site the
// same page lives at an address of its own under the site's base, so each such link is turned into that address here.

import path from 'node:path';
import { fileURLToPath } from 'node:url';

const PAGE_ENDINGS = /\.mdx?$/;
const HOME_PAGE = 'index';
const HAS_A_SCHEME_OR_A_ROOT = /^([a-z][a-z0-9+.-]*:|\/|#)/i;

function isLinkToAPageFile(address) {
	const [file] = address.split('#');
	return !HAS_A_SCHEME_OR_A_ROOT.test(address) && PAGE_ENDINGS.test(file);
}

// The page's place among the pages, such as `guides/first-plugin`, which is also how its address ends.
function placeAmongThePages(pageFile, pagesFolder) {
	const place = path.relative(pagesFolder, pageFile).replace(PAGE_ENDINGS, '').split(path.sep).join('/');
	return place === HOME_PAGE ? '' : place.replace(new RegExp(`/${HOME_PAGE}$`), '');
}

function addressOnTheSite(link, fromFile, { base, pagesFolder }) {
	const [file, heading] = link.split('#');
	const place = placeAmongThePages(path.resolve(path.dirname(fromFile), file), pagesFolder);
	const page = place ? `${base}/${place}/` : `${base}/`;
	return heading ? `${page}#${heading}` : page;
}

/**
 * A plugin for Astro's Markdown processor, to go in its `mdastPlugins`.
 *
 * @param {{ base: string, pagesFolder: string }} site `base` is the site's base without a closing slash, and
 * `pagesFolder` is the folder the pages are kept in.
 */
export function linksBetweenPages(site) {
	return ({ fileURL }) => {
		if (!fileURL) return null;
		const fromFile = fileURLToPath(fileURL);
		return {
			name: 'links-between-pages',
			link(node, context) {
				if (isLinkToAPageFile(node.url)) context.setProperty(node, 'url', addressOnTheSite(node.url, fromFile, site));
			},
		};
	};
}
