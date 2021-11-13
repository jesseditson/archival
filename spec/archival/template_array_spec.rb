# frozen_string_literal: true

require 'archival'

RSpec.describe Archival::TemplateArray do
  context 'normal array usage' do
    it 'can read with integers' do
      arr = Archival::TemplateArray.new(1, 'test')
      expect(arr[0]).to be('test')
    end
    it 'can write with integers' do
      arr = Archival::TemplateArray.new
      arr[0] = 'test'
      expect(arr[0]).to be('test')
    end
    it 'can push items and get ranges' do
      arr = Archival::TemplateArray.new
      arr.push('a')
      arr.push('b')
      arr.push('c')
      expect(arr[1..2]).to eq(%w[b c])
    end
  end
  context 'special subscripting' do
    it 'can set a key' do
      arr = Archival::TemplateArray.new
      arr['foo'] = 'test'
      expect(arr['foo']).to be('test')
    end
    it 'can set a symbol' do
      arr = Archival::TemplateArray.new
      arr[:foo] = 'test'
      expect(arr[:foo]).to be('test')
    end
    it 'handles .key?' do
      arr = Archival::TemplateArray.new
      arr[:foo] = 'test'
      expect(arr.key?(:foo)).to be(true)
    end
  end
end
