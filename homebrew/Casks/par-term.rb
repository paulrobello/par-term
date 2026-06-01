cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.32.2"
  sha256 arm:   "5aad2b4bd5b26d6f9ae867362e5a5b51017416afec881552622400c89e322515",
         intel: "feb2ba6da9fc1994201818f865d2b307d1a600a9993ff004383f02b7ee6a8a02"

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
