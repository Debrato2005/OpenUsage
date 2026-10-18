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
