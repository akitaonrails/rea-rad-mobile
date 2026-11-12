$input v_texcoord0

// license:MIT OR Apache-2.0
// Rad Mobile launcher: rea-crt chain, passes 2 and 3 (separable Gaussian).
// After Timothy Lottes' public-domain CRT shader; parameters after
// Wanszai's Scud Race PC port.
//
// Nine taps along u_blur_dir (1,0 = horizontal, 0,1 = vertical), one texel
// apart, with the classic 5-weight kernel (centre, then +-1..+-4).

#include "common.sh"

SAMPLER2D(s_tex, 0);

uniform vec4 u_tex_size0;
uniform vec4 u_blur_dir;

void main()
{
	vec2 dt = u_blur_dir.xy / u_tex_size0.xy;
	vec3 sum = texture2D(s_tex, v_texcoord0).rgb * 0.227027;
	sum += (texture2D(s_tex, v_texcoord0 + dt      ).rgb + texture2D(s_tex, v_texcoord0 - dt      ).rgb) * 0.194595;
	sum += (texture2D(s_tex, v_texcoord0 + dt * 2.0).rgb + texture2D(s_tex, v_texcoord0 - dt * 2.0).rgb) * 0.121622;
	sum += (texture2D(s_tex, v_texcoord0 + dt * 3.0).rgb + texture2D(s_tex, v_texcoord0 - dt * 3.0).rgb) * 0.054054;
	sum += (texture2D(s_tex, v_texcoord0 + dt * 4.0).rgb + texture2D(s_tex, v_texcoord0 - dt * 4.0).rgb) * 0.016216;
	gl_FragColor = vec4(sum, 1.0);
}
