cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.56.0"
  sha256 arm:   "0664de3fb4fa6c763d3f579909407e646610d2aff750de39d3aab4001bed017b",
         intel: "8ab38950e3ba1e92b79b263c143b1ffa90b17632287e9ebe7379ab168f3b62ab"

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
