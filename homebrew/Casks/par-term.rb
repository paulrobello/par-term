cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.10"
  sha256 arm:   "ff7bb5478df716b5284c3ff7490c6d96beb7eceac072676f345ccb8cac0c0bcb",
         intel: "cef96e13a4770b4bb4143c2e5b8c567db8f119de97a3cfadb88cc8412e3a6b20"

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
