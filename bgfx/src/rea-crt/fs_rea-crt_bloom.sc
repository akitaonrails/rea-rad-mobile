$input v_texcoord0

// license:MIT OR Apache-2.0
// Rad Mobile launcher: rea-crt chain, pass 1 (bloom extract).
// After Timothy Lottes' public-domain CRT shader; parameters after
// Wanszai's Scud Race PC port.
//
// Keeps only the bright part of the picture: k ramps from 0 at the
// threshold luminance (max channel) to 1 at full white.

#include "common.sh"

SAMPLER2D(s_tex, 0);

uniform vec4 u_bloom; // x = threshold

void main()
{
	vec3 c = texture2D(s_tex, v_texcoord0).rgb;
	float l = max(c.r, max(c.g, c.b));
	float k = saturate((l - u_bloom.x) / (1.0 - u_bloom.x));
	gl_FragColor = vec4(c * k, 1.0);
}
