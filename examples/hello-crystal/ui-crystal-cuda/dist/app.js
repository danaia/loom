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
uniform float zoom, growth, anisotropy, temperature, damage;
mat2 r(float a){float c=cos(a),s=sin(a);return mat2(c,-s,s,c);}
float shape(vec3 p){
  p.yz=r(rotation.y)*p.yz; p.xz=r(rotation.x)*p.xz;
  p.xz=r(.39)*p.xz;
  vec3 a=abs(p);
  float cube=max(a.x,max(a.y,a.z));
  float oct=(a.x+a.y+a.z)*.58;
  return max(cube,mix(oct,cube,anisotropy))-growth;
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
  outColor=vec4(pow(col,vec3(.82)),1.);
}`
function shader(type, source){const s=gl.createShader(type);gl.shaderSource(s,source);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw new Error(gl.getShaderInfoLog(s));return s}
const program=gl.createProgram();gl.attachShader(program,shader(gl.VERTEX_SHADER,vertex));gl.attachShader(program,shader(gl.FRAGMENT_SHADER,fragment));gl.linkProgram(program);gl.useProgram(program)
const buffer=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,buffer);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,3,-1,-1,3]),gl.STATIC_DRAW)
const pos=gl.getAttribLocation(program,'p');gl.enableVertexAttribArray(pos);gl.vertexAttribPointer(pos,2,gl.FLOAT,false,0,0)
const uniforms=Object.fromEntries(['resolution','rotation','zoom','growth','anisotropy','temperature','damage'].map(n=>[n,gl.getUniformLocation(program,n)]))
const state={rotation:[-.45,.25],zoom:1,growth:.72,anisotropy:.68,temperature:.18,damage:0}
let dragging=false,last=[0,0]
canvas.addEventListener('pointerdown',e=>{dragging=true;last=[e.clientX,e.clientY];canvas.setPointerCapture(e.pointerId)})
canvas.addEventListener('pointermove',e=>{if(!dragging)return;state.rotation[0]+=(e.clientX-last[0])*.008;state.rotation[1]+=(e.clientY-last[1])*.008;last=[e.clientX,e.clientY]})
canvas.addEventListener('pointerup',()=>dragging=false)
canvas.addEventListener('wheel',e=>{e.preventDefault();state.zoom=Math.min(2.5,Math.max(.55,state.zoom*Math.exp(-e.deltaY*.001)))},{passive:false})
for(const name of ['growth','anisotropy','temperature','damage']){
  const input=document.querySelector('#'+name), output=document.querySelector('#'+name+'Out')
  input.addEventListener('input',()=>{state[name]=Number(input.value);output.value=state[name].toFixed(2);invoke('set_control',{name:'crystal.'+name,value:state[name]})})
}
document.querySelector('#reset').addEventListener('click',()=>{for(const [name,value] of Object.entries({growth:.72,anisotropy:.68,temperature:.18,damage:0})){state[name]=value;document.querySelector('#'+name).value=value;document.querySelector('#'+name+'Out').value=value.toFixed(2);invoke('set_control',{name:'crystal.'+name,value})}})
async function connect(){try{const snap=await invoke('get_snapshot');if(snap?.values){for(const [key,value] of Object.entries(snap.values)){const name=key.replace('crystal.','');if(name in state){state[name]=value;document.querySelector('#'+name).value=value;document.querySelector('#'+name+'Out').value=Number(value).toFixed(2)}}}document.querySelector('#status').textContent='Connected'}catch{document.querySelector('#status').textContent='Viewer only'}}
function frame(){const dpr=devicePixelRatio||1,w=Math.floor(canvas.clientWidth*dpr),h=Math.floor(canvas.clientHeight*dpr);if(canvas.width!==w||canvas.height!==h){canvas.width=w;canvas.height=h}gl.viewport(0,0,w,h);gl.uniform2f(uniforms.resolution,w,h);gl.uniform2f(uniforms.rotation,...state.rotation);gl.uniform1f(uniforms.zoom,state.zoom);for(const n of ['growth','anisotropy','temperature','damage'])gl.uniform1f(uniforms[n],state[n]);gl.drawArrays(gl.TRIANGLES,0,3);requestAnimationFrame(frame)}
connect();frame()
