/// Fenwick prefix index for block lengths and layout heights.
#[derive(Clone,Default)]
pub(crate) struct PrefixIndex { values: Vec<f64>, tree: Vec<f64> }
impl PrefixIndex {
    pub(crate) fn rebuild(&mut self, values: impl IntoIterator<Item=f64>) { self.values=values.into_iter().collect();self.tree=vec![0.;self.values.len()+1];for index in 0..self.values.len(){let value=self.values[index];self.add(index,value);} }
    fn add(&mut self,index:usize,delta:f64){let mut node=index+1;while node<self.tree.len(){self.tree[node]+=delta;node+=node&node.wrapping_neg();}}
    pub(crate) fn set(&mut self,index:usize,value:f64){let delta=value-self.values[index];self.values[index]=value;self.add(index,delta);}
    pub(crate) fn sum(&self,end:usize)->f64{let mut node=end.min(self.values.len());let mut sum=0.;while node>0{sum+=self.tree[node];node&=node-1;}sum}
    pub(crate) fn total(&self)->f64{self.sum(self.values.len())}
    pub(crate) fn locate(&self,offset:f64)->usize{let mut index=0;let mut sum=0.;let mut bit=self.values.len().next_power_of_two();while bit>0{let next=index+bit;if next<self.tree.len()&&sum+self.tree[next]<=offset{index=next;sum+=self.tree[next];}bit>>=1;}index.min(self.values.len().saturating_sub(1))}
}
#[cfg(test)]
mod tests { use super::*; #[test] fn local_delta_updates_suffix_sums(){let mut index=PrefixIndex::default();index.rebuild([2.,3.,4.]);index.set(1,7.);assert_eq!(index.sum(2),9.);assert_eq!(index.total(),13.);assert_eq!(index.locate(9.),2);} }
