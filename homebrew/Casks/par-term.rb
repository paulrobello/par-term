cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.53.0"
  sha256 arm:   "9bea1fd993ccdeaee4ed90248dccee71a1aa8ddffe32af01797b82e3ef419a31",
         intel: "1cfdb988c069a72da8c42d508c23beda6ad462cd0f5cb8e26569294ac7a459b4"

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
