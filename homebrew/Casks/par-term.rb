cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.7"
  sha256 arm:   "8b659db44a2d4be637ff601546e138bdd26d4b5cad1608264b0aabb01ea9c163",
         intel: "82629a200147120701e30824ddb5d6e267a29e7b01fbc9b02361820b7019ab75"

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
