cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.14.0"
  sha256 arm:   "7aff3936c8604d67e32ec45cbaab678299a71838581a86baa419453f98ebcd99",
         intel: "8364bb4d59e8e1e1925365da8a1a7feba71fc53d69b2103520fe3aa89d156ad1"

  url "https://github.com/paulrobello/par-term/releases/download/v#{version}/par-term-macos-#{arch}.zip"
  name "par-term"
  desc "Cross-platform GPU-accelerated terminal emulator with inline graphics support"
  homepage "https://github.com/paulrobello/par-term"

  depends_on macos: ">= :catalina"

  livecheck do
    url :homepage
    strategy :github_latest
  end

  app "par-term.app"

  zap trash: [
    "~/Library/Application Support/par-term",
    "~/Library/Preferences/com.paulrobello.par-term.plist",
    "~/Library/Saved Application State/com.paulrobello.par-term.savedState",
    "~/.config/par-term",
  ]
end
