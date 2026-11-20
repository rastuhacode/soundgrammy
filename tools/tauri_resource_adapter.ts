// Audit-only adapter for the disposable develop checkout. UI components stay intact.
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
import { useFullscreenStore } from '@/stores/fullscreen-store'
const wait=(ms:number)=>new Promise(r=>setTimeout(r,ms))
const frame=()=>new Promise<number>(r=>requestAnimationFrame(r))
const click=(label:string)=>{const el=document.querySelector<HTMLElement>(`[aria-label="${label}"]`);if(!el)throw new Error(`Missing ${label}`);el.click()}
export async function startBenchmark(fixture:any) {
 const scenario=fixture?._benchmark_scenario
 if(!scenario)return
 const win=getCurrentWindow()
 try {
  await wait(1500)
  if(scenario==='search'||scenario==='search-filter') {
   await win.setSize(new LogicalSize(390,844));await wait(200)
   click('Select All tracks playlist');await frame();await frame()
  }
  const opened=performance.now()
  if(scenario==='search'||scenario==='search-filter') {click('Search tracks');await frame();await frame()}
  if(scenario==='search-filter') {
   const input=document.querySelector<HTMLInputElement>('.playlist-search-overlay input')!
   Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value')!.set!.call(input,'005')
   input.dispatchEvent(new Event('input',{bubbles:true}));await frame();await frame()
  }
  if(scenario==='drawer'||scenario==='settings') {click('Open navigation menu');await frame();await frame()}
  if(scenario==='settings') {
   await wait(400)
   const el=Array.from(document.querySelectorAll<HTMLElement>('button')).find(el=>el.textContent?.trim()==='Settings')
   if(!el)throw new Error('No Settings item');el.click()
  }
  if(scenario.startsWith('fullscreen'))await useFullscreenStore.getState().enterFullscreen()
  if(scenario==='minimized')await win.minimize()
  let opening:number|null=null
  if(scenario!=='minimized') {await frame();await frame();opening=performance.now()-opened}
  const ready={scenario,tracks:fixture.tracks.length,window:[innerWidth,innerHeight],fullscreen:useFullscreenStore.getState().isFullscreen,
    opening_callback_ms:opening,ready_text:document.body.innerText.slice(0,1000),
    artwork_loaded:Array.from(document.images).filter(i=>i.complete&&i.naturalWidth>0).length,
    matching_rows:document.querySelectorAll('[role="row"]').length,
    query:document.querySelector<HTMLInputElement>('.playlist-search-overlay input')?.value??null}
  if(scenario==='scroll'||scenario==='artwork-scroll'||scenario==='search') {
   const viewport=Array.from(document.querySelectorAll<HTMLElement>('[data-slot="scroll-area-viewport"]')).find(el=>el.querySelector('[role="rowgroup"]'))
   if(!viewport)throw new Error('No scrolling viewport')
   const started=performance.now();let last=started
   const scroll=(now:number)=>{const dt=Math.min((now-last)/1000,.1);last=now;viewport.scrollTop+=(Math.floor((now-started)/8000)%2===0?1:-1)*1080*dt;requestAnimationFrame(scroll)}
   requestAnimationFrame(scroll)
  }
  await wait(2000)
  await invoke('audit_ready',{payload:{...ready,phase:'ready'}})
  if(['scroll','artwork-scroll','search','fullscreen','fullscreen-playing'].includes(scenario)) {
   const times:number[]=[];const started=performance.now()
   const sample=(now:number)=>{times.push(now);if(now-started<8500)requestAnimationFrame(sample);else void invoke('audit_ready',{payload:{phase:'metrics',times_ms:times,method:'requestAnimationFrame callbacks; not actual compositor presentations'}})}
   requestAnimationFrame(sample)
  }
 } catch(error) {await invoke('audit_ready',{payload:{scenario,error:String(error)}})}
}
