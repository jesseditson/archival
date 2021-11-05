# frozen_string_literal: true

require 'archival'

RSpec.describe Archival do
  context 'basics' do
    it 'getting a listener works' do
      root = File.join(FIXTURES_DIR,
                       'simple_website')
      listener = Archival.listen(root)
      expect(listener).to exist?
    end
  end
end
