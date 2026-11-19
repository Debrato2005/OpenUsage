import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function settleViewport(driver, width, height) {
  return driver.request('POST', `/session/${driver.session}/execute/async`, {
    script: `const [width,height,done]=arguments;
      const read=()=>({width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth});
      const before=read(),deadline=performance.now()+5000,samples=[];let previous,stable=0;
      function frame(){
        document.documentElement.getBoundingClientRect();document.body.clientWidth;document.documentElement.clientWidth;
        const current=read();samples.push(current);if(samples.length>8)samples.shift();
        const valid=current.width===width&&current.height===height&&current.scrollWidth<=width+1;
        stable=valid?(previous&&JSON.stringify(previous)===JSON.stringify(current)?stable+1:1):0;previous=current;
        if(stable>=2||performance.now()>=deadline)return done({ok:stable>=2,before,samples,method:'Forced viewport layout and two matching animation-frame measurements; no CSS changes or fixed sleep.'});
        requestAnimationFrame(frame);
      }requestAnimationFrame(frame);`,
    args: [width, height],
  });
}
export async function captureHistoryThemes(driver, output, name) {
  const path = `/session/${driver.session}/window/rect`;
  const original = await driver.request('GET', path);
  const before = await driver.execute('return {width:innerWidth,height:innerHeight,theme:document.documentElement.dataset.theme ?? null};');
  const screenshots = [];
  try {
    await driver.request('POST', path, { width: original.width + 720 - before.width, height: original.height });
    for (const theme of ['light', 'dark']) {
      await driver.execute('document.documentElement.dataset.theme=arguments[0];[...document.querySelectorAll("[data-detail-panel][open] dt")].find(row=>row.textContent==="Reading status").scrollIntoView({block:"center"});', [theme]);
      const view = await settleViewport(driver, 720, before.height);
      assert.ok(view.ok, JSON.stringify(view));
      const file = `${name}-${theme}-720.png`;
      await driver.screenshot(join(output, file));
      screenshots.push(file);
    }
  } finally {
    await driver.execute('const html=document.documentElement;const theme=arguments[0];if(theme===null)delete html.dataset.theme;else html.dataset.theme=theme;', [before.theme]);
    await driver.request('POST', path, { width: original.width, height: original.height });
    const restored = await settleViewport(driver, before.width, before.height);
    assert.ok(restored.ok, JSON.stringify(restored));
  }
  return screenshots;
}
