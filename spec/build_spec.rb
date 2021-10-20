require 'build'

RSpec.describe Build, "#render" do
    context "basic content" do
        it "performs a simple substitution" do
            build = Build.new
            build.set_content("hi {{name}}")
            build.set_var("name", "Jesse")
            out = build.render()
            expect(out).to eq "hi Jesse"
        end
    end
end
