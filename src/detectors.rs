/// A detector recognizes a directory (by name, and optionally by a sibling
/// "marker" file that must exist alongside it) as a safe-to-delete,
/// regenerable build artifact.
#[derive(Debug, Clone, Copy)]
pub struct Detector {
    /// Human readable ecosystem/tool name, shown in output.
    pub label: &'static str,
    /// The directory name we're looking for (e.g. "node_modules").
    pub dir_name: &'static str,
    /// If set, only match when a file with this name exists as a sibling
    /// of the matched directory (helps avoid false positives on generic
    /// names like "build" or "dist").
    pub marker_sibling: Option<&'static str>,
    /// The command the user would run to regenerate this artifact.
    pub regenerate_hint: &'static str,
}

/// The full list of known reclaimable directories. Ordered roughly by how
/// commonly people hit them so early scan output "feels" relevant fast.
pub const DETECTORS: &[Detector] = &[
    Detector {
        label: "Node.js",
        dir_name: "node_modules",
        marker_sibling: None,
        regenerate_hint: "npm install / yarn / pnpm install",
    },
    Detector {
        label: "Rust",
        dir_name: "target",
        marker_sibling: Some("Cargo.toml"),
        regenerate_hint: "cargo build",
    },
    Detector {
        label: "Python venv",
        dir_name: ".venv",
        marker_sibling: None,
        regenerate_hint: "python -m venv .venv && pip install -r requirements.txt",
    },
    Detector {
        label: "Python venv",
        dir_name: "venv",
        marker_sibling: None,
        regenerate_hint: "python -m venv venv && pip install -r requirements.txt",
    },
    Detector {
        label: "Python cache",
        dir_name: "__pycache__",
        marker_sibling: None,
        regenerate_hint: "regenerated automatically on next run",
    },
    Detector {
        label: "Gradle",
        dir_name: ".gradle",
        marker_sibling: None,
        regenerate_hint: "./gradlew build",
    },
    Detector {
        label: "Gradle build output",
        dir_name: "build",
        marker_sibling: Some("build.gradle"),
        regenerate_hint: "./gradlew build",
    },
    Detector {
        label: "Gradle build output",
        dir_name: "build",
        marker_sibling: Some("build.gradle.kts"),
        regenerate_hint: "./gradlew build",
    },
    Detector {
        label: "Maven",
        dir_name: "target",
        marker_sibling: Some("pom.xml"),
        regenerate_hint: "mvn package",
    },
    Detector {
        label: "Xcode DerivedData",
        dir_name: "DerivedData",
        marker_sibling: None,
        regenerate_hint: "regenerated automatically on next Xcode build",
    },
    Detector {
        label: "CocoaPods",
        dir_name: "Pods",
        marker_sibling: Some("Podfile"),
        regenerate_hint: "pod install",
    },
    Detector {
        label: ".NET",
        dir_name: "bin",
        marker_sibling: Some(".csproj"),
        regenerate_hint: "dotnet build",
    },
    Detector {
        label: ".NET",
        dir_name: "obj",
        marker_sibling: Some(".csproj"),
        regenerate_hint: "dotnet build",
    },
    Detector {
        label: "Next.js",
        dir_name: ".next",
        marker_sibling: None,
        regenerate_hint: "next build",
    },
    Detector {
        label: "Nuxt",
        dir_name: ".nuxt",
        marker_sibling: None,
        regenerate_hint: "nuxt build",
    },
    Detector {
        label: "Terraform",
        dir_name: ".terraform",
        marker_sibling: None,
        regenerate_hint: "terraform init",
    },
    Detector {
        label: "CMake",
        dir_name: "cmake-build-debug",
        marker_sibling: None,
        regenerate_hint: "cmake --build .",
    },
    Detector {
        label: "Android Studio",
        dir_name: ".idea",
        marker_sibling: None,
        regenerate_hint: "regenerated automatically by the IDE",
    },
    Detector {
        label: "Composer (PHP)",
        dir_name: "vendor",
        marker_sibling: Some("composer.json"),
        regenerate_hint: "composer install",
    },
    Detector {
        label: "Go build cache",
        dir_name: "vendor",
        marker_sibling: Some("go.mod"),
        regenerate_hint: "go mod vendor",
    },
    Detector {
        label: "Ruby (Bundler)",
        dir_name: ".bundle",
        marker_sibling: Some("Gemfile"),
        regenerate_hint: "bundle install",
    },
    Detector {
        label: "Elixir",
        dir_name: "_build",
        marker_sibling: Some("mix.exs"),
        regenerate_hint: "mix compile",
    },
    Detector {
        label: "Elixir deps",
        dir_name: "deps",
        marker_sibling: Some("mix.exs"),
        regenerate_hint: "mix deps.get",
    },
    Detector {
        label: "Haskell (Stack)",
        dir_name: ".stack-work",
        marker_sibling: None,
        regenerate_hint: "stack build",
    },
    Detector {
        label: "Haskell (Cabal)",
        dir_name: "dist-newstyle",
        marker_sibling: None,
        regenerate_hint: "cabal build",
    },
    Detector {
        label: "Zig",
        dir_name: "zig-cache",
        marker_sibling: None,
        regenerate_hint: "zig build",
    },
    Detector {
        label: "Zig output",
        dir_name: "zig-out",
        marker_sibling: None,
        regenerate_hint: "zig build",
    },
    Detector {
        label: "Flutter / Dart",
        dir_name: ".dart_tool",
        marker_sibling: Some("pubspec.yaml"),
        regenerate_hint: "flutter pub get / dart pub get",
    },
    Detector {
        label: "Swift Package Manager",
        dir_name: ".build",
        marker_sibling: Some("Package.swift"),
        regenerate_hint: "swift build",
    },
];

/// Directory names we never recurse into, either because they're irrelevant
/// (version control internals) or because descending into an already-matched
/// artifact directory wastes time (we report it as one unit).
pub const SKIP_DESCENDING: &[&str] = &[".git", ".hg", ".svn"];
