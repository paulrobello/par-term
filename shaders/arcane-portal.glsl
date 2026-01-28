/*! par-term shader metadata
name: arcane-portal
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.25
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
*/

// Original by chronos: https://www.shadertoy.com/view/wf3BWM

const float H = 1.8;
const vec4 COS_OFFSETS = cos(vec4(1,2,2.5,0)) + 1.0; // Precomputed constant

float f(vec3 p)
{
    float sdf = p.y;
    float pz = p.z * 0.1;
    for(float j = .04; j < 6.; j+=j)
        sdf += (abs(dot(sin(pz + p/j), vec3(.2)))-.1)*j;
    return sdf;
}

float sabs(float x) { return sqrt(x*x+0.09)-0.3; } // a=0.3, a*a=0.09
float f2(vec3 p)
{
    float sdf = p.y;
        for(float j = 2.56; j < 6.; j+=j)
            sdf += (sabs(dot(sin(p.z*.1 + p/j), vec3(.2)))-.1)*j;
    sdf = min(sdf, p.y+H);
    return sdf;
}
vec4 portal_target(float time, in vec3 ro, in vec3 rd )
{
    vec4 fragColor;
    vec3 col = vec3(0);
    ro -= vec3(0,.8,4.*time);
    ro.x += -sin(time*.2) * 10.;
    float a = cos(time*.2)*.25;
    vec4 cv = cos(a + vec4(0,-11,11,0));
    rd.xy *= mat2(cv.x, cv.y, cv.z, cv.w);

    float t = 0.;
    ro.y += .2-1.5*f2(ro);

    ro.y = .5*(ro.y + H) + .5*sabs(ro.y + H)-H;

    float angle = .2 * (
        ((f2(ro) - f2(ro+vec3(0,0,1)))*.75+.15) +
        ((f2(ro+vec3(0,0,-.5)) - f2(ro+vec3(0,0,.5)))*.75+.15)
        );
    float C = cos(angle), S = sin(angle);
    mat2 M = mat2(C,S,-S,C);
    rd.yz *= M;

    int i = 0;
    float T = 1.;
    float sdf = 9e9;
    for(; i < 60 && t < 1e2; i++)
    {
        vec3 p = rd * t + ro;

        if(p.y < -H)
        {
            float fr = clamp(1.+rd.y,0.,1.);
            float fr2 = fr*fr;
            T = fr2*fr2*fr; // pow(fr, 5)
            p.y = abs(p.y+H)-H;
        }

        sdf = f(p);
        t += sdf*.65 + 1e-3;

        if(abs(sdf) < 1e-3)
        {
            vec2 e = vec2(0, 0.05);
            vec3 n = normalize(vec3(f(p+e.yxx), f(p+e.xyx), f(p+e.xxy))-sdf);
            float fr = clamp(1.+dot(n,rd),0.,1.);
            float fr2 = fr*fr;
            col += fr2*fr2*fr;
            break;
        }
        col += (.75+.25*sin(vec3(-1.75,2.5,1.3)+2.4*vec3(.3,.6,1)*sdf))*.1*sdf * exp2(-.5*sdf-.1*t) * T;
    }

    fragColor = vec4(col, 0);
    return fragColor;
}


float pi = 3.14159265;

vec3 triwave(vec3 x)
{
    return abs(fract(.5*x/pi-.25)-.5)*4.-1.;
}

void mainImage( out vec4 o, in vec2 fragCoord )
{
    vec2 r = iResolution.xy;
    vec2 fc = vec2(fragCoord.x, r.y - fragCoord.y); // Flip Y
    vec2 uv = (fc*2.-r) / r.y;
    float t = iTime, d, i, z = 0.;

    o = vec4(0,0,0,1);
    vec3 cam_pos = vec3(0, 1.5, 10.);

    vec3 rd = normalize(vec3(uv, -1.4));

    {
        float time = iTime*.25;
        cam_pos += vec3(1.5*cos(time), 0, 2.*sin(time));
        float angle = cos(time)*.25;
        float c = cos(angle), s = sin(angle);
        rd.xz *= mat2(c,s,-s,c);
    }

    vec3 P = vec3(0,2.3,2.5); // Portal pos
    float h = 1.; // ground height (negated :P )

    // intersect ground
    float g_hit_t = (-h - cam_pos.y)/rd.y;
    vec3 g_hit = g_hit_t * rd + cam_pos;

    // Figure out portal reflection
    vec3 portal_cam_pos = cam_pos;
    vec3 portal_rd = rd;
    if(g_hit_t > 0. && g_hit.z > P.z)
    {
        vec3 A = vec3(-1,-h, P.z)-cam_pos;
        vec3 B = vec3(1, -h, P.z)-cam_pos;
        portal_rd = reflect(portal_rd, normalize(cross(A,B)));
    }

    vec4 portal_target_color;
    vec3 P2 = vec3(0, -2.*h-2.3, 2.5); // Portal pos
    float radius = smoothstep(0.,2., iTime)*3.;
    if(min(
        length(dot(P - cam_pos, rd) * rd + cam_pos - P),
        length(dot(P2 - cam_pos, rd) * rd + cam_pos - P2)
        ) < radius
      )
    {
        portal_target_color = portal_target(iTime, portal_cam_pos, portal_rd);
    }

    portal_target_color *= portal_target_color * 300.;
    float D; // sdf for waves around portal
    float D2; // sdf for orange glow below portal
    vec3 p;   // current ray march pos
    float transmission = 1.;
    for(i = 0.; i++<50. && z < 1e3;  // Reduced from 65
      o += transmission*
          mix(
                (cos(d*10.+vec4(1,2,2.5,0))+1.)/d*z,
                portal_target_color,
                smoothstep(0.0, -0.2, max(length(p-P) -radius, (p.z-P.z)))
            )
            + 2.*(cos(-4.5*iTime+D*10.+vec4(1,2,2.5,0))+1.)*exp2(-D*D)*z
            + 10.*COS_OFFSETS*exp2(-abs(D2))*z
    )

    {
      p = z*rd;
      vec3 q; // distorted p

      p += cam_pos;
      D = length(p-P)-radius;

      D2 = length((p-vec3(P.x,-h,P.z))*vec3(1.5,10.,1.5))-radius;

      if(p.y < -h) // reflect portal edge
      {
        p.y = abs(p.y+h)-h;
        float fr = clamp(1.+rd.y,0.,1.);
        float fr2 = fr*fr;
        transmission = .8 * (0.15 + 0.85*fr2*fr2*fr);
      }
      else
      {
        transmission = 1.;
      }

      p.y += .24*sin(p.z*2. + iTime*2. - d*12.);

      float T = 2.5*t-d*14.;
      float c = cos(T), s = sin(T);
      q = p-P;
      q.xy *= mat2(c,s,-s,c);

      for(d=1.;d++<9.;) q += triwave((q*d+t*2.)).yzx/d;

      d = .1*abs(length(p-P)-radius) + abs(q.z)*.1;
      z += min(abs(p.y+h)*.4+.03, d);
    }

    o = o/1e4;
    o *= 1.-length(uv)*.2;
    o = sqrt(1.-exp(-1.5*o*o));
}
