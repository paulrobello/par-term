cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.48.0"
  sha256 arm:   "ae0af7caa7a81ac83fbc20187d2746b7fa56f861a9b467da440442273566558a",
         intel: "07f071e2260f61b2889c3bb64cccb03e414d18de06d89fd0e8817d82a99f269e"

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
