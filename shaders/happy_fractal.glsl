// "Happy Fractal" - based on "Fractal Cartoon" by Kali
// Modified: Nyan Cat replaced with happy face, music replaced with wave pattern

//#define SHOWONLYEDGES
#define HAPPYFACE
#define WAVES
//#define BORDER

#define RAY_STEPS 150

#define BRIGHTNESS 1.2
#define GAMMA 1.4
#define SATURATION .65

#define detail .001
#define t iTime*.5

const vec3 origin=vec3(-1.,.7,0.);
float det=0.0;

// Procedural wave pattern to replace music input (chaotic, 75% reduced amplitude)
float wavePattern(float time) {
    float wave1 = sin(time * 3.14159) * 0.125 + 0.125;
    float wave2 = sin(time * 5.77 + 1.618) * 0.075;
    float wave3 = sin(time * 7.93 + 2.718) * 0.05;
    float wave4 = sin(time * 1.41 + 0.577) * 0.1 + 0.1;
    float wave5 = sin(time * 11.3 + 3.14) * 0.04;
    float wave6 = sin(time * 0.618 * time) * 0.06; // time-varying frequency for chaos
    return clamp(wave1 + wave2 + wave3 + wave4 * 0.5 + wave5 + wave6, 0.0, 1.0);
}

// 2D rotation function
mat2 rot(float a) {
    return mat2(cos(a),sin(a),-sin(a),cos(a));
}

// "Amazing Surface" fractal
vec4 formula(vec4 p) {
    p.xz = abs(p.xz+1.)-abs(p.xz-1.)-p.xz;
    p.y-=.25;
    p.xy*=rot(radians(35.));
    p=p*2./clamp(dot(p.xyz,p.xyz),.2,1.);
    return p;
}

// Distance function
float de(vec3 pos) {
#ifdef WAVES
    pos.y+=sin(pos.z-t*6.)*.15; //waves!
#endif
    float hid=0.;
    vec3 tpos=pos;
    tpos.z=abs(3.-mod(tpos.z,6.));
    vec4 p=vec4(tpos,1.);
    for (int i=0; i<4; i++) {p=formula(p);}
    float fr=(length(max(vec2(0.),p.yz-1.5))-1.)/p.w;
    float ro=max(abs(pos.x+1.)-.3,pos.y-.35);
          ro=max(ro,-max(abs(pos.x+1.)-.1,pos.y-.5));
    pos.z=abs(.25-mod(pos.z,.5));
          ro=max(ro,-max(abs(pos.z)-.2,pos.y-.3));
          ro=max(ro,-max(abs(pos.z)-.01,-pos.y+.32));
    float d=min(fr,ro);
    return d;
}

// Camera path
vec3 path(float ti) {
    ti*=1.5;
    vec3  p=vec3(sin(ti),(1.-sin(ti*2.))*.5,-ti*5.)*.5;
    return p;
}

// Calc normals, and here is edge detection, set to variable "edge"
float edge=0.;
vec3 normal(vec3 p) {
    vec3 e = vec3(0.0,det*5.,0.0);

    float d1=de(p-e.yxx),d2=de(p+e.yxx);
    float d3=de(p-e.xyx),d4=de(p+e.xyx);
    float d5=de(p-e.xxy),d6=de(p+e.xxy);
    float d=de(p);
    edge=abs(d-0.5*(d2+d1))+abs(d-0.5*(d4+d3))+abs(d-0.5*(d6+d5));
    edge=min(1.,pow(edge,.55)*15.);
    return normalize(vec3(d1-d2,d3-d4,d5-d6));
}

// Rainbow trail behind the happy face
vec4 rainbow(vec2 p)
{
    float q = max(p.x,-0.1);
    float s = sin(p.x*7.0+t*70.0)*0.08;
    p.y+=s;
    p.y*=1.1;

    vec4 c;
    if (p.x>0.0) c=vec4(0,0,0,0); else
    if (0.0/6.0<p.y&&p.y<1.0/6.0) c= vec4(255,43,14,255)/255.0; else
    if (1.0/6.0<p.y&&p.y<2.0/6.0) c= vec4(255,168,6,255)/255.0; else
    if (2.0/6.0<p.y&&p.y<3.0/6.0) c= vec4(255,244,0,255)/255.0; else
    if (3.0/6.0<p.y&&p.y<4.0/6.0) c= vec4(51,234,5,255)/255.0; else
    if (4.0/6.0<p.y&&p.y<5.0/6.0) c= vec4(8,163,255,255)/255.0; else
    if (5.0/6.0<p.y&&p.y<6.0/6.0) c= vec4(122,85,255,255)/255.0; else
    if (abs(p.y)-.05<0.0001) c=vec4(0.,0.,0.,1.); else
    if (abs(p.y-1.)-.05<0.0001) c=vec4(0.,0.,0.,1.); else
        c=vec4(0,0,0,0);
    c.a*=.8-min(.8,abs(p.x*.08));
    c.xyz=mix(c.xyz,vec3(length(c.xyz)),.15);
    return c;
}

// SDF circle
float sdCircle(vec2 p, float r) {
    return length(p) - r;
}

// SDF for smile arc
float sdArc(vec2 p, float r, float thickness) {
    // Only draw bottom half of circle for smile
    if (p.y > 0.0) return 1000.0;
    float d = abs(length(p) - r) - thickness;
    return d;
}

// Happy face function - replaces nyan cat
vec4 happyFace(vec2 p)
{
    // Scale and center the face
    vec2 uv = p * 3.0;

    // Bouncing animation
    float bounce = sin(iTime * 8.0) * 0.05;
    uv.y -= bounce;

    // Wobble animation
    float wobble = sin(iTime * 6.0) * 0.03;
    uv.x += wobble;

    vec4 color = vec4(0.0);

    // Face background (yellow circle)
    float face = sdCircle(uv, 0.5);
    if (face < 0.0) {
        color = vec4(1.0, 0.85, 0.0, 1.0); // Bright yellow

        // Add subtle gradient for depth
        float gradient = 1.0 - length(uv) * 0.3;
        color.rgb *= gradient;
    }

    // Face outline
    if (abs(face) < 0.04) {
        color = vec4(0.8, 0.5, 0.0, 1.0); // Orange outline
    }

    // Left eye
    vec2 leftEyePos = uv - vec2(-0.18, 0.12);
    float leftEye = sdCircle(leftEyePos, 0.08);
    if (leftEye < 0.0) {
        color = vec4(0.1, 0.1, 0.1, 1.0); // Dark eye

        // Eye shine
        vec2 shinePos = leftEyePos - vec2(-0.02, 0.02);
        float shine = sdCircle(shinePos, 0.025);
        if (shine < 0.0) {
            color = vec4(1.0, 1.0, 1.0, 1.0);
        }
    }

    // Right eye
    vec2 rightEyePos = uv - vec2(0.18, 0.12);
    float rightEye = sdCircle(rightEyePos, 0.08);
    if (rightEye < 0.0) {
        color = vec4(0.1, 0.1, 0.1, 1.0); // Dark eye

        // Eye shine
        vec2 shinePos = rightEyePos - vec2(-0.02, 0.02);
        float shine = sdCircle(shinePos, 0.025);
        if (shine < 0.0) {
            color = vec4(1.0, 1.0, 1.0, 1.0);
        }
    }

    // Smile - arc shape
    vec2 smilePos = uv - vec2(0.0, -0.05);
    float smileOuter = sdCircle(smilePos, 0.28);
    float smileInner = sdCircle(smilePos, 0.22);
    float smileTop = smilePos.y + 0.05; // Cut off top part

    if (smileOuter < 0.0 && smileInner > 0.0 && smileTop < 0.0) {
        color = vec4(0.1, 0.1, 0.1, 1.0); // Dark smile
    }

    // Rosy cheeks (blush)
    float waveIntensity = wavePattern(iTime) * 0.3 + 0.5;

    vec2 leftCheek = uv - vec2(-0.32, -0.02);
    float leftBlush = sdCircle(leftCheek, 0.08);
    if (leftBlush < 0.0 && face < 0.0) {
        float blushAlpha = (1.0 - leftBlush / -0.08) * 0.5 * waveIntensity;
        color.rgb = mix(color.rgb, vec3(1.0, 0.4, 0.4), blushAlpha);
    }

    vec2 rightCheek = uv - vec2(0.32, -0.02);
    float rightBlush = sdCircle(rightCheek, 0.08);
    if (rightBlush < 0.0 && face < 0.0) {
        float blushAlpha = (1.0 - rightBlush / -0.08) * 0.5 * waveIntensity;
        color.rgb = mix(color.rgb, vec3(1.0, 0.4, 0.4), blushAlpha);
    }

    return color;
}

// Raymarching and 2D graphics
vec3 raymarch(in vec3 from, in vec3 dir)
{
    edge=0.;
    vec3 p, norm;
    float d=100.;
    float totdist=0.;
    for (int i=0; i<RAY_STEPS; i++) {
        if (d>det && totdist<25.0) {
            p=from+totdist*dir;
            d=de(p);
            det=detail*exp(.13*totdist);
            totdist+=d;
        }
    }
    vec3 col=vec3(0.);
    p-=(det-d)*dir;
    norm=normal(p);
#ifdef SHOWONLYEDGES
    col=1.-vec3(edge); // show wireframe version
#else
    col=(1.-abs(norm))*max(0.,1.-edge*.8); // set normal as color with dark edges
#endif
    totdist=clamp(totdist,0.,26.);
    dir.y-=.02;

    // Use wave pattern instead of music texture for sun size
    float waveValue = wavePattern(iTime * 0.5);
    float sunsize=7.-waveValue*5.; // responsive sun size based on waves

    float an=atan(dir.x,dir.y)+iTime*1.5; // angle for drawing and rotating sun
    float s=pow(clamp(1.0-length(dir.xy)*sunsize-abs(.2-mod(an,.4)),0.,1.),.1); // sun
    float sb=pow(clamp(1.0-length(dir.xy)*(sunsize-.2)-abs(.2-mod(an,.4)),0.,1.),.1); // sun border
    float sg=pow(clamp(1.0-length(dir.xy)*(sunsize-4.5)-.5*abs(.2-mod(an,.4)),0.,1.),3.); // sun rays
    float y=mix(.45,1.2,pow(smoothstep(0.,1.,.75-dir.y),2.))*(1.-sb*.5); // gradient sky

    // set up background with sky and sun
    vec3 backg=vec3(0.5,0.,1.)*((1.-s)*(1.-sg)*y+(1.-sb)*sg*vec3(1.,.8,0.15)*3.);
         backg+=vec3(1.,.9,.1)*s;
         backg=max(backg,sg*vec3(1.,.9,.5));

    col=mix(vec3(1.,.9,.3),col,exp(-.004*totdist*totdist));// distant fading to sun color
    if (totdist>25.) col=backg; // hit background
    col=pow(col,vec3(GAMMA))*BRIGHTNESS;
    col=mix(vec3(length(col)),col,SATURATION);
#ifdef SHOWONLYEDGES
    col=1.-vec3(length(col));
#else
    col*=vec3(1.,.9,.85);
#ifdef HAPPYFACE
    dir.yx*=rot(dir.x);
    vec2 facePos=(dir.xy+vec2(-3.+mod(-t,6.),-.27));
    vec4 face=happyFace(facePos*3. + vec2(0., -0.05));  // 50% bigger, shifted up relative to rainbow
    vec4 rain=rainbow(facePos*10.+vec2(.3,.25));  // moved closer to face
    if (totdist>8.) col=mix(col,max(vec3(.2),rain.xyz),rain.a*.9);
    if (totdist>8.) col=mix(col,max(vec3(.2),face.xyz),face.a*.9);
#endif
#endif
    return col;
}

// get camera position
vec3 move(inout vec3 dir) {
    vec3 go=path(t);
    vec3 adv=path(t+.7);
    float hd=de(adv);
    vec3 advec=normalize(adv-go);
    float an=adv.x-go.x; an*=min(1.,abs(adv.z-go.z))*sign(adv.z-go.z)*.7;
    dir.xy*=mat2(cos(an),sin(an),-sin(an),cos(an));
    an=advec.y*1.7;
    dir.yz*=mat2(cos(an),sin(an),-sin(an),cos(an));
    an=atan(advec.x,advec.z);
    dir.xz*=mat2(cos(an),sin(an),-sin(an),cos(an));
    return go;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Flip Y to match Shadertoy coordinate system
    fragCoord.y = iResolution.y - fragCoord.y;
    vec2 uv = fragCoord.xy / iResolution.xy*2.-1.;
    vec2 oriuv=uv;
    uv.y*=iResolution.y/iResolution.x;
    vec2 mouse=(iMouse.xy/iResolution.xy-.5)*3.;
    if (iMouse.z<1.) mouse=vec2(0.,-0.05);
    float fov=.9-max(0.,.7-iTime*.3);
    vec3 dir=normalize(vec3(uv*fov,1.));
    dir.yz*=rot(mouse.y);
    dir.xz*=rot(mouse.x);
    vec3 from=origin+move(dir);
    vec3 color=raymarch(from,dir);
    #ifdef BORDER
    color=mix(vec3(0.),color,pow(max(0.,.95-length(oriuv*oriuv*oriuv*vec2(1.05,1.1))),.3));
    #endif
    fragColor = vec4(color*0.5,1.);
}
