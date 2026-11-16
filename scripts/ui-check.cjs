// Drives the box's UI in a headless browser at the display's size and saves screenshots.
// Read-only: it types a search and jumps the artist list, never presses Play.
// Usage: ssh -f -N -L 18686:localhost:8686 rpi
//        node scripts/ui-check.cjs <dir with node_modules/playwright> <screenshot dir>
const path = require('path');
const nm = process.argv[2];
const { chromium } = require(path.join(nm, 'playwright'));
const out = process.argv[3];
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 800, height: 480 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto('http://localhost:18686/');
  await page.waitForSelector('#app:not([hidden])', { timeout: 15000 });
  await page.waitForTimeout(1500);
  // The box's screen may be dark: the first touch only wakes it.
  await page.mouse.click(400, 300);
  await page.waitForTimeout(1000);
  await page.click('[data-tab="search"]');
  for (const k of ['b', 'l', 'u']) await page.click(`.keyboard button:text-is("${k}")`);
  await page.waitForTimeout(2500);
  await page.screenshot({ path: `${out}/s-search-typing.png` });
  await page.click('.keyboard button:text-is("ěšč 123")');
  await page.screenshot({ path: `${out}/s-search-cz.png` });
  await page.click('.keyboard button:text-is("Hide")');
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${out}/s-search-results.png` });
  await page.click('[data-tab="music"]');
  await page.click('.subnav button:text-is("Artists")');
  await page.waitForTimeout(1500);
  await page.click('button:text-is("Jump to letter")');
  await page.screenshot({ path: `${out}/s-letters.png` });
  await page.click('.letters-grid button:text-is("M")');
  await page.waitForTimeout(2000);
  await page.screenshot({ path: `${out}/s-artists-m.png` });
  console.log(errors.length ? 'ERRORS:\n' + errors.join('\n') : 'no page errors');
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); process.exit(1); });
