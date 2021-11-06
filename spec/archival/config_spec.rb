# frozen_string_literal: true

require 'archival'

RSpec.describe Archival::Config do
  context 'init' do
    it 'inits with defaults when not provided a config' do
      config = Archival::Config.new
      expect(config).to be_a(Archival::Config)
      expect(config.pages_dir).to be_a(String)
      expect(config.objects_dir).to be_a(String)
      expect(config.root).to be_a(String)
      expect(config.build_dir).to be_a(String)
      expect(config.helper_port).to be_a(Integer)
    end
    it 'is not :dev_mode by default' do
      config = Archival::Config.new
      expect(config.dev_mode).to be(false)
    end
    it 'accepts a :dev_mode symbol' do
      config = Archival::Config.new(dev_mode: true)
      expect(config.dev_mode).to be(true)
    end
  end
end
