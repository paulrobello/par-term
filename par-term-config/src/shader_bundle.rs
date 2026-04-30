//! Shader bundle manifest parsing and validation.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

/// Manifest describing a shader bundle and its local assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShaderBundleManifest {
    pub shader: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub license: String,
    #[serde(default)]
    pub textures: Vec<String>,
    #[serde(default)]
    pub cubemaps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawShaderBundleManifest {
    shader: Option<String>,
    name: Option<String>,
    author: Option<String>,
    description: Option<String>,
    license: Option<String>,
    #[serde(default)]
    textures: Vec<String>,
    #[serde(default)]
    cubemaps: Vec<String>,
    #[serde(default)]
    screenshot: Option<String>,
}

impl ShaderBundleManifest {
    /// Parse a JSON manifest string and report all missing required fields together.
    pub fn from_json_str(input: &str) -> Result<Self, String> {
        let raw: RawShaderBundleManifest = serde_json::from_str(input)
            .map_err(|e| format!("parse shader bundle manifest: {e}"))?;
        let missing = missing_required_fields(
            raw.shader.as_deref(),
            raw.name.as_deref(),
            raw.author.as_deref(),
            raw.description.as_deref(),
            raw.license.as_deref(),
        );
        if !missing.is_empty() {
            return Err(format!(
                "missing required shader bundle manifest field(s): {}",
                missing.join(", ")
            ));
        }

        Ok(Self {
            shader: raw.shader.expect("checked required shader"),
            name: raw.name.expect("checked required name"),
            author: raw.author.expect("checked required author"),
            description: raw.description.expect("checked required description"),
            license: raw.license.expect("checked required license"),
            textures: raw.textures,
            cubemaps: raw.cubemaps,
            screenshot: raw.screenshot,
        })
    }

    /// Validate required fields are present and non-empty.
    pub fn validate_required_fields(&self) -> Result<(), String> {
        let missing = missing_required_fields(
            Some(&self.shader),
            Some(&self.name),
            Some(&self.author),
            Some(&self.description),
            Some(&self.license),
        );
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "missing required shader bundle manifest field(s): {}",
                missing.join(", ")
            ))
        }
    }

    /// Validate that manifest asset paths are relative to `bundle_dir` and exist.
    pub fn validate_paths(&self, bundle_dir: &Path) -> Result<(), String> {
        self.validate_required_fields()?;

        validate_relative_path("shader", &self.shader)?;
        if !self.shader.ends_with(".glsl") {
            return Err(
                "shader bundle manifest field `shader` must point to a .glsl file".to_string(),
            );
        }
        ensure_exists(bundle_dir, "shader", &self.shader)?;

        for texture in &self.textures {
            validate_relative_path("textures", texture)?;
            ensure_exists(bundle_dir, "textures", texture)?;
        }

        for cubemap in &self.cubemaps {
            validate_relative_path("cubemaps", cubemap)?;
            ensure_cubemap_faces_exist(bundle_dir, cubemap)?;
        }

        if let Some(screenshot) = &self.screenshot {
            validate_relative_path("screenshot", screenshot)?;
            ensure_exists(bundle_dir, "screenshot", screenshot)?;
        }

        Ok(())
    }
}

fn missing_required_fields(
    shader: Option<&str>,
    name: Option<&str>,
    author: Option<&str>,
    description: Option<&str>,
    license: Option<&str>,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if shader.is_none_or(|value| value.trim().is_empty()) {
        missing.push("shader");
    }
    if name.is_none_or(|value| value.trim().is_empty()) {
        missing.push("name");
    }
    if author.is_none_or(|value| value.trim().is_empty()) {
        missing.push("author");
    }
    if description.is_none_or(|value| value.trim().is_empty()) {
        missing.push("description");
    }
    if license.is_none_or(|value| value.trim().is_empty()) {
        missing.push("license");
    }
    missing
}

fn validate_relative_path(field: &str, value: &str) -> Result<(), String> {
    let path = Path::new(value);
    let invalid_component = path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if value.trim().is_empty() || path.is_absolute() || invalid_component {
        return Err(format!(
            "shader bundle manifest field `{field}` must be a non-empty relative path without '..'"
        ));
    }
    Ok(())
}

fn ensure_exists(bundle_dir: &Path, field: &str, value: &str) -> Result<(), String> {
    if bundle_dir.join(value).is_file() {
        Ok(())
    } else {
        Err(format!(
            "shader bundle manifest field `{field}` path is not a file: {value}"
        ))
    }
}

fn ensure_cubemap_faces_exist(bundle_dir: &Path, prefix: &str) -> Result<(), String> {
    const SUFFIXES: [&str; 6] = ["px", "nx", "py", "ny", "pz", "nz"];
    const EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "hdr"];

    for suffix in SUFFIXES {
        let found = EXTENSIONS.iter().any(|ext| {
            bundle_dir
                .join(format!("{prefix}-{suffix}.{ext}"))
                .is_file()
        });
        if !found {
            return Err(format!(
                "missing cubemap face for prefix `{prefix}` and suffix `{suffix}`"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_manifest_requires_author_and_description() {
        let json = r#"{
            "shader": "shader.glsl",
            "name": "Missing Fields",
            "license": "MIT"
        }"#;

        let err = ShaderBundleManifest::from_json_str(json)
            .expect_err("missing author and description should fail");

        assert!(err.contains("author"));
        assert!(err.contains("description"));
    }

    #[test]
    fn validates_bundle_manifest_paths_relative_to_bundle_dir() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("shader.glsl"),
            "void mainImage(out vec4 c, in vec2 p){c=vec4(0.0);}",
        )
        .unwrap();
        std::fs::create_dir_all(temp.path().join("textures")).unwrap();
        std::fs::write(temp.path().join("textures/noise.png"), b"fake").unwrap();

        let manifest = ShaderBundleManifest {
            shader: "shader.glsl".to_string(),
            name: "Valid Bundle".to_string(),
            author: "par-term".to_string(),
            description: "A valid test bundle.".to_string(),
            license: "MIT".to_string(),
            textures: vec!["textures/noise.png".to_string()],
            cubemaps: Vec::new(),
            screenshot: None,
        };

        manifest.validate_paths(temp.path()).unwrap();
    }

    #[test]
    fn rejects_absolute_and_parent_bundle_paths() {
        let manifest = ShaderBundleManifest {
            shader: "/tmp/shader.glsl".to_string(),
            name: "Invalid Bundle".to_string(),
            author: "par-term".to_string(),
            description: "Invalid absolute shader path.".to_string(),
            license: "MIT".to_string(),
            textures: vec!["../noise.png".to_string()],
            cubemaps: Vec::new(),
            screenshot: None,
        };

        let err = manifest
            .validate_paths(std::path::Path::new("."))
            .expect_err("absolute shader path should fail");

        assert!(err.contains("relative path"));
    }

    #[test]
    fn rejects_shader_path_that_is_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("shader.glsl")).unwrap();
        let manifest = ShaderBundleManifest {
            shader: "shader.glsl".to_string(),
            name: "Invalid Bundle".to_string(),
            author: "par-term".to_string(),
            description: "Shader path is a directory.".to_string(),
            license: "MIT".to_string(),
            textures: Vec::new(),
            cubemaps: Vec::new(),
            screenshot: None,
        };

        let err = manifest
            .validate_paths(temp.path())
            .expect_err("shader directory should fail validation");

        assert!(err.contains("shader"));
    }

    #[test]
    fn rejects_texture_path_that_is_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("shader.glsl"), "void mainImage(){}").unwrap();
        std::fs::create_dir_all(temp.path().join("textures/noise.png")).unwrap();
        let manifest = ShaderBundleManifest {
            shader: "shader.glsl".to_string(),
            name: "Invalid Bundle".to_string(),
            author: "par-term".to_string(),
            description: "Texture path is a directory.".to_string(),
            license: "MIT".to_string(),
            textures: vec!["textures/noise.png".to_string()],
            cubemaps: Vec::new(),
            screenshot: None,
        };

        let err = manifest
            .validate_paths(temp.path())
            .expect_err("texture directory should fail validation");

        assert!(err.contains("textures"));
    }

    #[test]
    fn rejects_screenshot_path_that_is_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("shader.glsl"), "void mainImage(){}").unwrap();
        std::fs::create_dir(temp.path().join("screenshot.png")).unwrap();
        let manifest = ShaderBundleManifest {
            shader: "shader.glsl".to_string(),
            name: "Invalid Bundle".to_string(),
            author: "par-term".to_string(),
            description: "Screenshot path is a directory.".to_string(),
            license: "MIT".to_string(),
            textures: Vec::new(),
            cubemaps: Vec::new(),
            screenshot: Some("screenshot.png".to_string()),
        };

        let err = manifest
            .validate_paths(temp.path())
            .expect_err("screenshot directory should fail validation");

        assert!(err.contains("screenshot"));
    }

    #[test]
    fn shader_bundle_shader_path_must_be_glsl() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("shader.wgsl"), "// wrong extension").unwrap();
        let manifest = ShaderBundleManifest {
            shader: "shader.wgsl".to_string(),
            name: "Invalid Bundle".to_string(),
            author: "par-term".to_string(),
            description: "Invalid shader extension.".to_string(),
            license: "MIT".to_string(),
            textures: Vec::new(),
            cubemaps: Vec::new(),
            screenshot: None,
        };

        let err = manifest
            .validate_paths(temp.path())
            .expect_err("non-GLSL shader should fail");

        assert!(err.contains(".glsl"));
    }

    #[test]
    fn validates_cubemap_prefix_requires_all_six_faces() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("shader.glsl"), "void mainImage(){}").unwrap();
        for suffix in ["px", "nx", "py", "ny", "pz"] {
            std::fs::write(temp.path().join(format!("env-{suffix}.png")), b"fake").unwrap();
        }
        let manifest = ShaderBundleManifest {
            shader: "shader.glsl".to_string(),
            name: "Cubemap Bundle".to_string(),
            author: "par-term".to_string(),
            description: "Cubemap validation test.".to_string(),
            license: "MIT".to_string(),
            textures: Vec::new(),
            cubemaps: vec!["env".to_string()],
            screenshot: None,
        };

        let err = manifest
            .validate_paths(temp.path())
            .expect_err("missing nz cubemap face should fail");

        assert!(err.contains("nz"));
    }

    #[test]
    fn rejects_cubemap_face_path_that_is_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("shader.glsl"), "void mainImage(){}").unwrap();
        for suffix in ["px", "nx", "py", "ny", "pz", "nz"] {
            std::fs::create_dir(temp.path().join(format!("env-{suffix}.png"))).unwrap();
        }
        let manifest = ShaderBundleManifest {
            shader: "shader.glsl".to_string(),
            name: "Cubemap Bundle".to_string(),
            author: "par-term".to_string(),
            description: "Cubemap validation test.".to_string(),
            license: "MIT".to_string(),
            textures: Vec::new(),
            cubemaps: vec!["env".to_string()],
            screenshot: None,
        };

        let err = manifest
            .validate_paths(temp.path())
            .expect_err("cubemap face directories should fail validation");

        assert!(err.contains("px"));
    }
}
