const canvas = document.querySelector('#crystal')
const gl = canvas.getContext('webgl2', { antialias: true })
const invoke = (command, args = {}) => window.__TAURI_INTERNALS__?.invoke(command, args)

const vertex = `#version 300 es
in vec2 p; void main(){ gl_Position=vec4(p,0.,1.); }`
const fragment = `#version 300 es
precision highp float;
out vec4 outColor;
uniform vec2 resolution;
uniform vec2 rotation;
uniform float zoom, growth, anisotropy, temperature, damage, cutStrength;
uniform float showField, showParticles, particleCount;
uniform vec2 cutDirection;
mat2 r(float a){float c=cos(a),s=sin(a);return mat2(c,-s,s,c);}
float shape(vec3 p){
  p.yz=r(rotation.y)*p.yz; p.xz=r(rotation.x)*p.xz;
  p.xz=r(.39)*p.xz;
  vec3 a=abs(p);
  float cube=max(a.x,max(a.y,a.z));
  float oct=(a.x+a.y+a.z)*.58;
  float crystal=max(cube,mix(oct,cube,anisotropy))-growth;
  // Difference the crystal with a thin plane. The plane is supplied by a
  // crystal stroke, so the cut remains attached to the specimen as it rotates.
  float cut=abs(dot(p.xy,normalize(cutDirection)))-(.012+.028*cutStrength);
  // Blend from the untouched distance field: multiplying the subtraction by
  // zero would otherwise turn the entire field into a zero-distance surface.
  return mix(crystal,max(crystal,-cut),cutStrength);
}
vec3 normal(vec3 p){float e=.002;return normalize(vec3(shape(p+vec3(e,0,0))-shape(p-vec3(e,0,0)),shape(p+vec3(0,e,0))-shape(p-vec3(0,e,0)),shape(p+vec3(0,0,e))-shape(p-vec3(0,0,e))));}
void main(){
  vec2 uv=(gl_FragCoord.xy*2.-resolution)/resolution.y;
  vec3 ro=vec3(0.,0.,2.8/zoom), rd=normalize(vec3(uv,-1.7));
  float t=0.; bool hit=false; vec3 p;
  for(int i=0;i<110;i++){p=ro+rd*t;float d=shape(p);if(d<.001){hit=true;break;}t+=max(d*.58,.004);if(t>6.)break;}
  vec3 bg=mix(vec3(.012,.025,.035),vec3(.025,.075,.10),max(0.,1.-length(uv))*.5);
  if(!hit){outColor=vec4(bg,1);return;}
  vec3 n=normal(p), l=normalize(vec3(-.5,.8,.65)), v=-rd;
  float dif=.2+.8*max(dot(n,l),0.), fre=pow(1.-abs(dot(n,v)),3.);
  float glint=pow(max(dot(reflect(-l,n),v),0.),30.);
  vec3 cold=mix(vec3(.12,.58,.88),vec3(.48,.93,1.),temperature);
  vec3 col=mix(cold,vec3(1.,.04,.015),damage*.88)*dif;
  col=mix(col,vec3(.72,.96,1.),fre*.5)+glint;
  float grid=.5+.5*sin((p.x*73.+p.y*41.-p.z*57.)*8.);
  col*=.82+.18*grid;
  // The cubic-root density maps the chosen count to the same 3D lattice as
  // the 100³ simulation field. Particle-only mode reveals individual cells;
  // "both" keeps the field and adds bright lattice markers on its surface.
  float density=pow(max(particleCount,1.),1./3.);
  vec3 cell=fract((p+vec3(1.5))*density)-.5;
  float particle=1.-smoothstep(.11,.24,length(cell));
  if(showField<.5 && (showParticles<.5 || particle<.12)){outColor=vec4(bg,1);return;}
  col*=showField;
  col+=vec3(.78,.96,1.)*particle*showParticles*(.55+.45*fre);
  outColor=vec4(pow(col,vec3(.82)),1.);
}`
function shader(type, source){const s=gl.createShader(type);gl.shaderSource(s,source);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw new Error(gl.getShaderInfoLog(s));return s}
const program=gl.createProgram();gl.attachShader(program,shader(gl.VERTEX_SHADER,vertex));gl.attachShader(program,shader(gl.FRAGMENT_SHADER,fragment));gl.linkProgram(program);gl.useProgram(program)
const buffer=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,buffer);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,3,-1,-1,3]),gl.STATIC_DRAW)
const pos=gl.getAttribLocation(program,'p');gl.enableVertexAttribArray(pos);gl.vertexAttribPointer(pos,2,gl.FLOAT,false,0,0)
const uniforms=Object.fromEntries(['resolution','rotation','zoom','growth','anisotropy','temperature','damage','cutStrength','cutDirection','showField','showParticles','particleCount'].map(n=>[n,gl.getUniformLocation(program,n)]))
const state={rotation:[-.45,.25],zoom:1,growth:.08,growthTarget:.72,anisotropy:.68,temperature:.18,damage:0,cutStrength:0,cutDirection:[1,0],showField:1,showParticles:0,particleCount:1000000}
let dragging=false, cutting=false, last=[0,0], previousFrame=performance.now()
function isOverCrystal(event){
  const box=canvas.getBoundingClientRect(), x=(event.clientX-box.left)/box.width*2-1, y=(event.clientY-box.top)/box.height*2-1
  // A conservative hit area lets users orbit freely from the background while
  // making the visible crystal reliably sliceable without a CPU ray marcher.
  return x*x+(y*box.height/box.width)*(y*box.height/box.width)<.34
}
canvas.addEventListener('pointerdown',e=>{dragging=true;cutting=isOverCrystal(e);last=[e.clientX,e.clientY];canvas.setPointerCapture(e.pointerId);if(cutting)canvas.classList.add('cutting')})
canvas.addEventListener('pointermove',e=>{if(!dragging)return;const dx=e.clientX-last[0],dy=e.clientY-last[1];if(cutting&&(Math.abs(dx)+Math.abs(dy)>2)){const length=Math.hypot(dx,dy)||1;state.cutDirection=[-dy/length,dx/length];state.cutStrength=1;state.damage=Math.max(state.damage,.72);document.querySelector('#damage').value=state.damage;document.querySelector('#damageOut').value=state.damage.toFixed(2)}else if(!cutting){state.rotation[0]+=dx*.008;state.rotation[1]+=dy*.008}last=[e.clientX,e.clientY]})
function endDrag(){dragging=false;cutting=false;canvas.classList.remove('cutting')}
canvas.addEventListener('pointerup',endDrag)
canvas.addEventListener('pointercancel',endDrag)
canvas.addEventListener('wheel',e=>{e.preventDefault();state.zoom=Math.min(2.5,Math.max(.55,state.zoom*Math.exp(-e.deltaY*.001)))},{passive:false})
for(const name of ['growth','anisotropy','temperature','damage']){
  const input=document.querySelector('#'+name), output=document.querySelector('#'+name+'Out')
  input.addEventListener('input',()=>{const value=Number(input.value);if(name==='growth')state.growthTarget=value;else state[name]=value;output.value=value.toFixed(2);invoke('set_control',{name:'crystal.'+name,value})})
}
for(const name of ['showField','showParticles']){
  document.querySelector('#'+name).addEventListener('change',event=>{state[name]=event.target.checked?1:0})
}
document.querySelector('#particleCount').addEventListener('input',event=>{state.particleCount=Number(event.target.value);document.querySelector('#particleCountOut').value=state.particleCount.toLocaleString();invoke('set_control',{name:'crystal.particle_count',value:state.particleCount})})
document.querySelector('#reset').addEventListener('click',()=>{for(const [name,value] of Object.entries({growth:.72,anisotropy:.68,temperature:.18,damage:0})){if(name==='growth'){state.growth=.08;state.growthTarget=value}else state[name]=value;document.querySelector('#'+name).value=value;document.querySelector('#'+name+'Out').value=value.toFixed(2);invoke('set_control',{name:'crystal.'+name,value})}state.cutStrength=0})
async function connect(){try{const snap=await invoke('get_snapshot');if(snap?.values){for(const [key,value] of Object.entries(snap.values)){const name=key.replace('crystal.','');if(name==='growth'){state.growthTarget=value}else if(name in state){state[name]=value}if(name in state){document.querySelector('#'+name).value=value;document.querySelector('#'+name+'Out').value=Number(value).toFixed(2)}}}document.querySelector('#status').textContent='Connected'}catch{document.querySelector('#status').textContent='Viewer only'}}
function frame(now){const dt=Math.min(.05,(now-previousFrame)/1000);previousFrame=now;state.growth+=(state.growthTarget-state.growth)*(1-Math.exp(-dt*.7));state.cutStrength=Math.max(0,state.cutStrength-dt*.22);state.damage=Math.max(0,state.damage-dt*.075);const dpr=devicePixelRatio||1,w=Math.floor(canvas.clientWidth*dpr),h=Math.floor(canvas.clientHeight*dpr);if(canvas.width!==w||canvas.height!==h){canvas.width=w;canvas.height=h}gl.viewport(0,0,w,h);gl.uniform2f(uniforms.resolution,w,h);gl.uniform2f(uniforms.rotation,...state.rotation);gl.uniform1f(uniforms.zoom,state.zoom);gl.uniform1f(uniforms.growth,state.growth);for(const n of ['anisotropy','temperature','damage','cutStrength','showField','showParticles','particleCount'])gl.uniform1f(uniforms[n],state[n]);gl.uniform2f(uniforms.cutDirection,...state.cutDirection);gl.drawArrays(gl.TRIANGLES,0,3);requestAnimationFrame(frame)}
connect();requestAnimationFrame(frame)
