#version 450

struct Instance
{
	mat4 model;
	vec4 arrayIndex;
};

layout (binding = 0) uniform UBO 
{
	mat4 projection;
	mat4 view;
	Instance instance[8];
} ubo;


layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;
layout(location = 2) in vec2 inTexCoord;

layout (location = 0) out vec3 outTexCoord;

void main() 
{
	outTexCoord = vec3(inTexCoord, ubo.instance[gl_InstanceIndex].arrayIndex.x);
	mat4 modelView = ubo.view * ubo.instance[gl_InstanceIndex].model;
	gl_Position = ubo.projection * modelView * vec4(inPosition, 1.0);
}
