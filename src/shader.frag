#version 100
precision mediump float;
varying vec2 uv;

uniform vec2 u_resolution; // Screen resolution for aspect ratio correction
uniform float u_time; // Time from main program
uniform vec3 u_camera_position; // Camera position from main program
uniform vec3 u_camera_direction; // Camera direction from main program

// Ray structure
struct Ray {
    vec3 origin;
    vec3 direction;
};

// Sphere structure
struct Sphere {
    vec3 center;
    float radius;
};

// Ray-sphere intersection function
float intersectSphere(Ray ray, Sphere sphere) {
    vec3 oc = ray.origin - sphere.center;
    float a = dot(ray.direction, ray.direction);
    float b = 2.0 * dot(oc, ray.direction);
    float c = dot(oc, oc) - sphere.radius * sphere.radius;
    float discriminant = b * b - 4.0 * a * c;
    
    if (discriminant < 0.0) {
        return -1.0; // No intersection
    } else {
        return (-b - sqrt(discriminant)) / (2.0 * a); // Return nearest intersection distance
    }
}

// Generate a ray from camera through pixel
Ray generateRay(vec2 uv) {
    // Adjust for aspect ratio
    float aspect = u_resolution.x / u_resolution.y;
    vec2 normalizedUV = uv * 2.0 - 1.0; // Convert 0-1 to -1 to 1
    normalizedUV.x *= aspect;
    
    Ray ray;
    ray.origin = u_camera_position; // Use camera position from main program
    
    // Calculate ray direction based on camera direction and pixel coordinates
    // First, find the camera's right and up vectors
    vec3 forward = normalize(u_camera_direction);
    vec3 right = normalize(cross(forward, vec3(0.0, 1.0, 0.0)));
    vec3 up = normalize(cross(right, forward));
    
    // Create view plane
    float fov = 60.0 * 3.14159 / 180.0; // Field of view in radians
    float tanFov = tan(fov * 0.5);
    
    // Calculate ray direction through pixel
    ray.direction = normalize(
        forward + 
        right * normalizedUV.x * tanFov + 
        up * normalizedUV.y * tanFov
    );
    
    return ray;
}

void main() {
    // Create a ray through the current pixel
    Ray ray = generateRay(uv);
    
    // Define a sphere in the center of the scene
    Sphere sphere;
    sphere.center = vec3(0.0, 0.0, 0.0);
    sphere.radius = 0.5;
    
    // Add a second sphere that moves over time
    Sphere sphere2;
    sphere2.center = vec3(sin(u_time) * 1.5, 0.0, cos(u_time) * 1.5);
    sphere2.radius = 0.3;
    
    // Check for intersection with both spheres
    float t1 = intersectSphere(ray, sphere);
    float t2 = intersectSphere(ray, sphere2);
    
    // Determine which sphere is closer (if any)
    float t = -1.0;
    bool isSphere1 = false;
    
    if (t1 > 0.0 && (t2 <= 0.0 || t1 < t2)) {
        t = t1;
        isSphere1 = true;
    } else if (t2 > 0.0) {
        t = t2;
        isSphere1 = false;
    }
    
    if (t > 0.0) {
        // Calculate hit point
        vec3 hitPoint = ray.origin + ray.direction * t;
        
        // Calculate normal at hit point and color based on which sphere was hit
        vec3 normal;
        vec3 sphereColor;
        
        if (isSphere1) {
            normal = normalize(hitPoint - sphere.center);
            sphereColor = vec3(1.0, 0.2, 0.2); // Red for first sphere
        } else {
            normal = normalize(hitPoint - sphere2.center);
            sphereColor = vec3(0.2, 1.0, 0.2); // Green for second sphere
        }
        
        // Simple lighting - direction to light
        vec3 lightDir = normalize(vec3(1.0, 1.0, 1.0));
        
        // Diffuse lighting
        float diff = max(dot(normal, lightDir), 0.0);
        vec3 diffuse = sphereColor * diff;
        
        // Add ambient light
        vec3 ambient = vec3(0.1, 0.1, 0.1);
        
        // Final color
        vec3 color = ambient + diffuse;
        gl_FragColor = vec4(color, 1.0);
    } else {
        // Background color (blue gradient based on y coordinate)
        vec3 backgroundColor = vec3(0.1, 0.2, 0.8) * (1.0 - uv.y * 0.5);
        gl_FragColor = vec4(backgroundColor, 1.0);
    }
}
