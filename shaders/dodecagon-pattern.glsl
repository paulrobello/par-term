/*
    Dodecagon Quad Pattern
    ----------------------
    https://www.shadertoy.com/view/t3yyzD

    Dodecagon-based quad pattern with overlapping frame and BRDF materials.
*/

// PI and 2 PI.
#define PI 3.14159265
#define TAU 6.2831853

////// Variable Defines ///////

// Frame color -- Silver: 0, Copper: 1, Gold: 2.
#define FRAME_COL 2

// Frame style -- Single: 0, Double: 1.
#define FRAME_STYLE 0

// Display the metal rivots.
#define RIVOTS

// Beveling the tops of the frames.
#define BEVELED

// Rounded polygon edges.
#define ROUNDED

// Dodecagon subdivision.
#define DOD_SUB 2

// Concave octagon subdivision.
#define OCT_SUB 1

/////// Constant Defines ///////

// Far plane.
#define FAR 20.

// Loop anti-unrolling hack.
#define ZERO min(int(iFrame), 0)

/////////////////////
// Common tab functions
/////////////////////

// Standard 2D rotation formula.
mat2 rot2(in float a){ float c = cos(a), s = sin(a); return mat2(c, s, -s, c); }

// Commutative smooth minimum function.
float smin(float a, float b, float k){
   float f = max(0., 1. - abs(b - a)/k);
   return min(a, b) - k*.25*f*f;
}

// 2D vector version of smin.
vec2 smin2(vec2 a, vec2 b, float k){
   vec2 f = max(vec2(0), 1. - abs(b - a)/k);
   return min(a, b) - k*.25*f*f;
}

// Hash function - 1 out, 2 in.
float hash21(vec2 p){
    vec3 p3  = fract(vec3(p.xyx)*.1031);
    p3 += dot(p3, p3.yzx + 42.123);
    return fract((p3.x + p3.y) * p3.z);
}

// Hash function - 1 out, 3 in.
float hash31(vec3 p3){
    p3  = fract(p3*vec3(.6031, .5030, .4973));
    p3 += dot(p3, p3.zyx + 43.527);
    return fract((p3.x + p3.y) * p3.z);
}

// Signed line distance.
float distLineS(vec2 p, vec2 a, vec2 b){
    b -= a;
    return dot(p - a, vec2(-b.y, b.x)/length(b));
}

// BRDF helper functions.
float GGX_Schlick(float nv, float rough) {
    float r = .5 + .5*rough;
    float k = (r*r)/2.;
    float denom = nv*(1. - k) + k;
    return max(nv, .001)/denom;
}

float G_Smith(float nr, float nl, float rough) {
    float g1_l = GGX_Schlick(nl, rough);
    float g1_v = GGX_Schlick(nr, rough);
    return g1_l*g1_v;
}

vec3 getSpec(vec3 FS, float nh, float nr, float nl, float rough){
    float alpha = pow(rough, 4.);
    float b = (nh*nh*(alpha - 1.) + 1.);
    float D = alpha/(PI*b*b);
    float G = G_Smith(nr, nl, rough);
    return FS*D*G/(4.*max(nr, .001))*PI;
}

vec3 getDiff(vec3 FS, float nl, float rough, float type){
    vec3 diff = nl*(1. - FS);
    return diff*(1. - type);
}

// IQ's regular polygon distance formula.
float sdRegularPolygon(in vec2 p, in float r, in int n){
    float an = PI/float(n);
    vec2  acs = vec2(cos(an), sin(an));
    float bn = mod(atan(p.x, p.y) + PI/12., 2.*an) - an;
    p = length(p)*vec2(cos(bn), abs(sin(bn)));
    p -= r*acs;
    p.y += clamp( -p.y, 0., r*acs.y);
    return length(p)*sign(p.x);
}

/////////////////////
// Main shader code
/////////////////////

// Tri-Planar blending function using iChannel1.
vec3 tex3D_ch1(in vec3 p, in vec3 n){
    n = max(n*n - .2, .001);
    n /= dot(n, vec3(1));
    vec3 tx = texture(iChannel1, p.zy).xyz;
    vec3 ty = texture(iChannel1, p.xz).xyz;
    vec3 tz = texture(iChannel1, p.xy).xyz;
    return mat3(tx*tx, ty*ty, tz*tz)*n;
}

// IQ's 2D box function.
float sBoxS(in vec2 p, in vec2 b, in float rf){
    vec2 d = abs(p) - b + rf;
    return min(max(d.x, d.y), 0.) + length(max(d, 0.)) - rf;
}

// Path function.
vec2 path(in float z){
    float a = sin(z*.11);
    float b = cos(z*.14);
    return vec2(a*2. - b*1.5, 0);
}

// IQ's extrusion formula.
float opExtrusion(in float sdf, in float pz, in float h, in float sf){
    vec2 w = vec2(sdf, abs(pz) - h) + sf;
    return min(max(w.x, w.y), 0.) + length(max(w, 0.)) - sf;
}

// Polygon distance.
#define NV 12
float sdPoly(in vec2 p, in vec2[NV] v, int num){
    float d = length(p - v[0]);
    for(int i = 0, j = num - 1; i < num; j = i, i++){
        vec2 e = v[j] - v[i];
        vec2 w = p - v[i];
        vec2 b = w - e*clamp(dot(w, e)/dot(e, e), 0., 1.);
        #ifdef ROUNDED
        d = smin(d, length(b), .06);
        #else
        d = min(d, length(b));
        #endif
    }
    return -d;
}

// Edge and vertex ID values.
const mat4x2 vID = mat4x2(vec2(-.5, -.5), vec2(-.5, .5), vec2(.5, .5), vec2(.5, -.5));
const mat4x2 eID = mat4x2(vec2(-.5, 0), vec2(0, .5), vec2(.5, 0), vec2(0, -.5));

// Overall scale.
const vec3 oSc = vec3(2);

// Polygon type ID.
int polyID = -1;

// Dodecahedron and polygon vertex containers.
vec2[12] vDod;
vec2[12] vP;

// Surface ID.
int gID;

// Global 2D distance.
float gD2;

// Number of polygon vertices.
int pID;

const float apoth = oSc.x/2./cos(TAU/24.);

// Precalculating the dodecagon vertices.
void preCalDodecagon(){
    for(int i = 0; i<12; i++){
        vDod[i] = rot2(-float(i)*TAU/12. + TAU/24.)*vec2(-apoth, 0);
    }
}

vec4 getGrid(vec2 p, inout vec2 sc){
    vec2 ip = floor(p/sc) + .5;
    p -= ip*sc;

    float dodeca = sdRegularPolygon(p, apoth, 12);

    if(dodeca<0.){
        vP = vDod;
        pID = 12;
        polyID = 4;

        #if DOD_SUB>0
        float ang = atan(p.y, -p.x)/TAU + 1.5/12.;
        int i = int(mod(ang*6., 6.));

        float ln0 = distLineS(p, vec2(0), vP[(i*2 + 11)%12]);
        float ln1 = distLineS(p, vec2(0), vP[(i*2 + 1)%12]);

        pID = 4;
        vP[0] = vec2(0);
        vP[1] = vDod[(i*2 + 11)%12];
        vP[2] = vDod[(i*2 + 0)%12];
        vP[3] = vDod[(i*2 + 1)%12];

        vec2[12] svVP = vP;
        ip += mix(vP[0], vP[2], 2./3.)/sc;
        #endif

        #if DOD_SUB>1
        pID = 4;
        vec2 cntr = mix(vP[0], vP[2], 2./3.);
        vec2 m0 = mix(vP[0], vP[1], .5);
        vec2 m1 = mix(vP[0], vP[3], .5);

        vec3 ln3;
        ln3.x = distLineS(p, cntr, m0);
        ln3.y = distLineS(p, cntr, vP[2]);
        ln3.z = distLineS(p, cntr, m1);
        ln3 = max(-ln3, ln3.zxy);

        vP[0] = cntr;

        if(ln3.x<0.){
            vP[1] = m1;
            vP[2] = vec2(0);
            vP[3] = m0;
            ip -= cntr/2./sc;
            polyID = 0;
        }
        else if(ln3.y<0.){
            ip += mix(cntr, vP[1], .5)/sc;
            vP[3] = vP[2];
            vP[2] = vP[1];
            vP[1] = m0;
            polyID = 1;
        }
        else if(ln3.z<0.){
            ip += mix(cntr, vP[3], .5)/sc;
            vP[1] = vP[2];
            vP[2] = vP[3];
            vP[3] = m1;
            polyID = 2;
        }
        #endif
    }
    else {
        pID = 8;
        polyID = 3;

        float ang = atan(p.y, -p.x)/TAU;
        int i = int(mod(ang*4. + 1., 4.));
        ip += vID[i];

        int i3 = i*3;

        vP[0] = vDod[i3];
        vP[1] = vDod[(i3 + 11)%12];
        vP[2] = vDod[(i3 + 3)%12] + eID[(i + 3)%4]*sc*2.;
        vP[3] = vDod[(i3 + 2)%12] + eID[(i + 3)%4]*sc*2.;
        vP[4] = vDod[(i3 + 6)%12] + vID[(i + 0)%4]*sc*2.;
        vP[5] = vDod[(i3 + 5)%12] + vID[(i + 0)%4]*sc*2.;
        vP[6] = vDod[(i3 + 9)%12] + eID[(i + 0)%4]*sc*2.;
        vP[7] = vDod[(i3 + 8)%12] + eID[(i + 0)%4]*sc*2.;

        #if OCT_SUB>0
        pID = 4;
        vec2[12] svVP = vP;

        vec2 cntr = vID[i]*sc;
        vec2 q = p - cntr;
        ang = atan(q.y, -q.x)/TAU;

        int iQuad = int(mod(ang*4.- .5, 4.));
        ip += eID[iQuad]/4.;

        iQuad = (iQuad + i3)*2;

        vP[0] = cntr;
        vP[1] = svVP[(iQuad + 7)%8];
        vP[2] = svVP[(iQuad)%8];
        vP[3] = svVP[(iQuad + 1)%8];

        float ln0 = distLineS(p, cntr, vP[1]);
        float ln1 = distLineS(p, cntr, vP[3]);

        polyID = 3;
        #endif
    }

    float d = sdPoly(p, vP, pID);
    gD2 = d;

    return vec4(p, ip*sc);
}

vec4 gVal;

float map(vec3 p3){
    vec2 sc = oSc.xz;
    vec4 p4 = getGrid(p3.xz, sc);
    float d2 = gD2;

    #if FRAME_STYLE == 1
    d2 = abs(d2 + .025) - .02;
    #else
    d2 = abs(d2 - .005) - .04;
    #endif

    float h = .1;
    float th = h/2.;

    vec2 pth = path(p4.w);
    float pY = p3.y - pth.y + 2.;

    float fl = pY + (gD2 + .025)*.25;
    #if DOD_SUB>0
    fl += gD2/sc.x*.5;
    #endif

    float d = opExtrusion(d2, pY - h, th, .01);
    float dE = opExtrusion(abs(gD2) - .05, pY - h, th - .002, .01);

    #ifdef BEVELED
    d += d2*.2;
    #endif

    #ifdef RIVOTS
    float dV2 = 1e5;
    for(int i = 0; i<pID; i++){
        dV2 = min(dV2, length(p4.xy - vP[i]) - .035);
    }
    float dV = opExtrusion(dV2, pY - h, th + .025, .01);

    dE = min(dE, dV - .015);
    dE = max(dE, -dV2);
    d = min(d, dV + dV2*.15);
    #endif

    gVal = p4;
    gID = d<fl && d<dE ? 0 : fl<dE? 1 : 2;

    return min(d, min(fl, dE));
}

float trace(in vec3 ro, in vec3 rd){
    float d, t = 0.;
    for(int i = ZERO; i<180; i++){
        d = map(ro + rd*t);
        if(abs(d)<.001 || t>FAR) break;
        t += d*.8;
    }
    return min(t, FAR);
}

vec3 normal(in vec3 p) {
    float sgn = 1.;
    vec3 e = vec3(.001, 0, 0), mp = e.zzz;
    for(int i = ZERO; i<6; i++){
        mp.x += map(p + sgn*e)*sgn;
        sgn = -sgn;
        if((i&1)==1){ mp = mp.yzx; e = e.zxy; }
    }
    return normalize(mp);
}

float softShadow(vec3 ro, vec3 rd, vec3 n, float lDist, float k){
    ro += n*.0015;
    ro += rd*hash31(ro + n*57.13)*.01;

    float shade = 1.;
    float t = 0.;

    for (int i = min(0, int(iFrame)); i<48; i++){
        float d = map(ro + rd*t);
        shade = min(shade, k*d/t);
        if (d<0. || t>lDist) break;
        t += clamp(d, .01, .2);
    }

    return max(shade, 0.);
}

float calcAO(in vec3 p, in vec3 n){
    float sca = 2., occ = 0.;
    for(int i = 0; i<5; i++){
        float hr = float(i + 1)*.15/5.;
        float d = map(p + n*hr);
        occ += (hr - d)*sca;
        sca *= .75;
    }
    return clamp(1. - occ, 0., 1.);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord){
    vec2 uv = (fragCoord - iResolution.xy*.5)/iResolution.y;

    vec3 lookAt = vec3(0, 0, iTime/2.);
    vec3 camPos = lookAt + vec3(0, 1, -.2);
    vec3 lightPos = camPos + vec3(0, 0, 2);

    lookAt.xy += path(lookAt.z);
    camPos.xy += path(camPos.z);
    lightPos.xy += path(lightPos.z);

    float FOV = TAU/6.;
    vec3 forward = normalize(lookAt - camPos);
    vec3 right = normalize(vec3(forward.z, 0, -forward.x));
    vec3 up = cross(forward, right);

    mat3 cam = mat3(right, up, forward);
    vec3 rd = cam*normalize(vec3(uv, 1./FOV));

    preCalDodecagon();

    float t = trace(camPos, rd);

    int svGID = gID;
    vec4 svVal = gVal;
    int svPolyID = polyID;

    vec2 cntr = vec2(0);
    float ln = 1e5;
    float cir = 1e5;
    for(int i = 0; i<pID; i++){
        cntr += vP[i]/float(pID);
    }
    svVal.xy -= cntr;
    cir = min(cir, length(svVal.xy));

    float dst = min(cir - .04, ln - .01);

    vec3 sky = vec3(1, .7, .4);
    vec3 sceneCol = sky;

    if(t<FAR){
        vec3 sp = camPos + t*rd;
        vec3 sn = normal(sp);

        vec3 ld = lightPos - sp;
        float lDist = max(length(ld), 1e-5);
        ld /= lDist;

        float atten = 1./(1. + lDist*lDist*.05);

        float ao = calcAO(sp, sn);
        float sh = softShadow(sp, ld, sn, lDist, 8.);

        float fresRef = .5;
        float type = .2;
        float rough = 1.;

        vec2 id = svVal.zw;

        vec3 txP = sp;
        txP.xy *= rot2(-PI/4.);
        vec3 tx = tex3D_ch1(txP/1., sn);
        float gr = dot(tx, vec3(.299, .587, .114));

        float rnd = hash21(id + .1);
        float range = hash21(id + .011);
        float saturation = .7;
        vec3 texCol = .5 + .45*cos(TAU*rnd/6. + vec3(0, PI/2., PI)*saturation);

        if(svGID==0){
            #if FRAME_COL == 0
            texCol = mix(vec3(.33, .3, .27)*1.25, texCol.zyx, .25);
            if(sp.y>-1.84) texCol = texCol*.5;
            #elif FRAME_COL == 1
            texCol = mix(vec3(.6, .25, .3), texCol, .25);
            if(sp.y>-1.84) texCol = texCol*vec3(.5, .7, 1);
            #else
            texCol = mix(vec3(1, .7, .4)*.5, texCol*1., .35);
            if(sp.y>-1.84) texCol = texCol*.5;
            #endif
            texCol *= tx*1.;
        }
        else if(svGID==2){
            texCol = texCol*.1;
            texCol *= tx;
        }
        else{
            texCol = texCol.yzx;
            if(svPolyID==0 || svPolyID==3){
                if(svPolyID==0) texCol = texCol.xzy;
                else texCol = mix(texCol, texCol.zyx*1.4, .6);
            }
            texCol = mix(texCol, texCol*.02, 1. - smoothstep(0., 1./450., dst));
            texCol *= tx*3. + .1;
        }

        float amb = length(sin(sn*2.)*.5 + .5)/sqrt(3.)*smoothstep(-1., 1., sn.y);

        float bac = clamp(dot(sn, -normalize(vec3(ld.x, 0, ld.z))), 0., 1.);
        texCol += texCol*bac*.5;

        if(svGID==1){
            fresRef = .25;
            rough = min(gr*4., 1.);
            type = .2;
        }
        else {
            type = .8;
            fresRef = .5;
            rough = min(gr*4., 1.);
        }

        vec3 h = normalize(ld - rd);
        float ndl = dot(sn, ld);
        float nr = clamp(dot(sn, -rd), 0., 1.);
        float nl = clamp(ndl, 0., 1.);
        float nh = clamp(dot(sn, h), 0., 1.);
        float vh = clamp(dot(-rd, h), 0., 1.);

        vec3 f0 = vec3(.16*(fresRef*fresRef));
        f0 = mix(f0, texCol, type);
        vec3 FS = f0 + (1. - f0)*pow(1. - vh, 5.);

        vec3 spec = getSpec(FS, nh, nr, nl, rough);
        vec3 diff = getDiff(FS, nl, rough, type);

        float bl = max(dot(-normalize(vec3(ld.x, 0, ld.z)), sn), 0.);
        texCol = texCol + texCol*sky*bl*2.;

        texCol += texCol*sky*(sn.y*.35 + .65);

        sceneCol = texCol*(diff*sh + amb*(sh*.5 + .5) + vec3(8)*spec*sh);

        float speR = pow(max(dot(normalize(ld - rd), sn), 0.), 8.);
        vec3 rf = reflect(rd, sn);
        vec3 rTx = texture(iChannel2, rf.xy).xyz; rTx *= rTx;
        float rF = svGID==1? 2. : svGID==0? 16. : 6.;
        sceneCol = sceneCol + sceneCol*speR*rTx*rF;

        sceneCol *= atten*ao;
    }

    sceneCol = mix(sceneCol, sky, smoothstep(.2, 1., t/FAR));

    fragColor = vec4(pow(max(sceneCol, 0.), vec3(1)/2.2), 1.0);
}
