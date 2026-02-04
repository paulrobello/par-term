cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.9.0"
  sha256 arm:   "ee22f25d003814efb5c6fae5ba9f8558290903fcc55f7f144461a31c6729ab53",
         intel: "5a5b7c4779d7688dc9fabdb8b22ab1c80ff4ea5a33f2d2e6e68378f72c52a530"

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
