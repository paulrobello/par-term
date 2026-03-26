cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.29.2"
  sha256 arm:   "a3c52a814e755bfd5f2ee034889b6a06c47ed0ae0f293302c1ba6de30451186a",
         intel: "96e936f7a925a0aca220b6748667386ac3708861819c4df2ff8326f7c7b8cec5"

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
