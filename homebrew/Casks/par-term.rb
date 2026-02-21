cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.21.0"
  sha256 arm:   "4981398d607648330b56757411f351532cfe6d9531be3c0e592ddebcab1ec525",
         intel: "b20589e1673f8844994bd625f46ddeab8f001a8a94919133fda550c464ffef19"

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
