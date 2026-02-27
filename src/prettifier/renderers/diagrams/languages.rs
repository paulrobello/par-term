//! Diagram language registry: supported diagram types and their metadata.

/// A supported diagram language with rendering metadata.
#[derive(Debug, Clone)]
pub struct DiagramLanguage {
    /// The fenced code block tag (e.g., "mermaid", "plantuml", "dot").
    pub tag: String,
    /// Display name (e.g., "Mermaid", "PlantUML").
    pub display_name: String,
    /// Kroki API type identifier (if supported by Kroki).
    pub kroki_type: Option<String>,
    /// Local CLI command to render this language.
    pub local_command: Option<String>,
    /// Arguments for local CLI command.
    pub local_args: Vec<String>,
}

/// Return the default set of diagram languages.
pub fn default_diagram_languages() -> Vec<DiagramLanguage> {
    vec![
        DiagramLanguage {
            tag: "mermaid".into(),
            display_name: "Mermaid".into(),
            kroki_type: Some("mermaid".into()),
            local_command: Some("mmdc".into()),
            local_args: vec![
                "-i".into(),
                "/dev/stdin".into(),
                "-o".into(),
                "/dev/stdout".into(),
                "-e".into(),
                "png".into(),
            ],
        },
        DiagramLanguage {
            tag: "plantuml".into(),
            display_name: "PlantUML".into(),
            kroki_type: Some("plantuml".into()),
            local_command: Some("plantuml".into()),
            local_args: vec!["-tpng".into(), "-pipe".into()],
        },
        DiagramLanguage {
            tag: "graphviz".into(),
            display_name: "GraphViz".into(),
            kroki_type: Some("graphviz".into()),
            local_command: Some("dot".into()),
            local_args: vec!["-Tpng".into()],
        },
        DiagramLanguage {
            tag: "dot".into(),
            display_name: "GraphViz".into(),
            kroki_type: Some("graphviz".into()),
            local_command: Some("dot".into()),
            local_args: vec!["-Tpng".into()],
        },
        DiagramLanguage {
            tag: "d2".into(),
            display_name: "D2".into(),
            kroki_type: Some("d2".into()),
            local_command: Some("d2".into()),
            local_args: vec!["-".into(), "-".into()],
        },
        DiagramLanguage {
            tag: "ditaa".into(),
            display_name: "Ditaa".into(),
            kroki_type: Some("ditaa".into()),
            local_command: None,
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "svgbob".into(),
            display_name: "SvgBob".into(),
            kroki_type: Some("svgbob".into()),
            local_command: Some("svgbob".into()),
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "erd".into(),
            display_name: "Erd".into(),
            kroki_type: Some("erd".into()),
            local_command: Some("erd".into()),
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "vegalite".into(),
            display_name: "Vega-Lite".into(),
            kroki_type: Some("vegalite".into()),
            local_command: None,
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "wavedrom".into(),
            display_name: "WaveDrom".into(),
            kroki_type: Some("wavedrom".into()),
            local_command: None,
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "excalidraw".into(),
            display_name: "Excalidraw".into(),
            kroki_type: Some("excalidraw".into()),
            local_command: None,
            local_args: vec![],
        },
    ]
}
