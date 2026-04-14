#!/usr/bin/env wolframscript

(* ::Package:: *)

(* ::Title:: *)
(*Permutation Puzzle Position Order Tallies*)
(*Orignal source from Lucas Garron: https://github.com/lgarron/permutation-order-tallies*)


CyclePossibilities[n_,o_]:=CyclePossibilities[n,o]=Catenate[Table[{i,twist},{i,n},{twist,0,o-1}]]


CyclePossibilitiesUpToLast[n_,o_,Null]:=CyclePossibilities[n,o]
CyclePossibilitiesUpToLast[n_,o_,last_]:=CyclePossibilitiesUpToLast[n,o,last]=If[
n<last[[1]],
CyclePossibilities[n,o],
Take[CyclePossibilities[n,o],Position[CyclePossibilities[n,o],last][[1,1]]]
]


CyclePatterns[0,_,_]={{}};
CyclePatterns[n_,o_,last_:Null]:=CyclePatterns[n,o,last]=Catenate[Table[Table[Catenate[{{first},rest}],{rest,CyclePatterns[n-first[[1]],o,first]}],{first,CyclePossibilitiesUpToLast[n,o,last]}]]


IsOrientationParityValid[o_][c_]:=Mod[Total[c[[All,2]]],o]==0


PermutationParity[c_]:=Mod[Total[c[[All,1]]-1],2]
HasValidOrientationParityAndGivenPermParity[o_,p_][c_]:=IsOrientationParityValid[o][c]&&PermutationParity[c]==p


CyclesWithValidOrientationParity[n_,o_]:=Select[CyclePatterns[n,o],IsOrientationParityValid[o]]
CyclesWithValidOrientationParityAndGivenPermParity[n_,o_,Null]:=CyclesWithValidOrientationParity[n,o]
CyclesWithValidOrientationParityAndGivenPermParity[n_,o_,p_]:=Select[CyclePatterns[n,o],HasValidOrientationParityAndGivenPermParity[o,p]]


NumPositions[n_,o_][c_]:=n!*o^n/Product[(tallyEntry[[1,1]]*o)^tallyEntry[[2]]*tallyEntry[[2]]!,{tallyEntry,Tally[c]}]


CycleOrder[n_,o_][{nc_,0}]:=nc
CycleOrder[n_,o_][{nc_,oc_}]:=nc*LCM[o,oc]/oc
CyclePatternOrder[n_,o_][cyclePattern_]:=CyclePatternOrder[n,o][cyclePattern]=LCM@@(CycleOrder[n,o]/@cyclePattern)


GatherTallySorted[l_]:=SortBy[{#[[1,1]],#[[All,2]]//Total}&/@GatherBy[l,First],First]


CycleTalliesWithPermParity[n_,o_,p_]:=CycleTalliesWithPermParity[n,o,p]=GatherTallySorted[{CyclePatternOrder[n,o][#],NumPositions[n,o][#]}&/@CyclesWithValidOrientationParityAndGivenPermParity[n,o,p]]


FoldOrbitTalliesWithCartesianExplosion[l_]:=GatherTallySorted[{LCM@@#[[All,1]],Times@@#[[All,2]]}&/@Tuples[l]]
CombineOrbitTallies[l_]:=Fold[FoldOrbitTalliesWithCartesianExplosion[{#1,#2}]&,CycleTalliesWithPermParity@@#&/@l]
CombineOrbitMultiTallies[l_]:=GatherTallySorted@Catenate[CombineOrbitTallies/@l]


AddOrbit[l__List,{n_Integer,o_Integer}]:=Flatten[Table[Append[s,{n,o,p}],{p,0,1},{s,l}],1]
AddOrbit[l_List,{n_Integer,o_Integer,p_Integer}]:=Table[Append[s,{n,o,p}],{s,l}]
AddOrbit[l_List,{n_Integer,o_Integer,xorOf_List}]:=Table[Append[s,{n,o,BitXor@@s[[xorOf,-1]]}],{s,l}]
CalculateOrbits[l_List]:=Fold[AddOrbit,{{}},l]
CalculateOrders[l_List]:=CombineOrbitMultiTallies[CalculateOrbits[l]]


WeightedMean[l_List]:=(WeightedData@@Transpose[l])//Mean//N


tally3x3x3=CalculateOrders[{
{8,3}, (* corners*)
{12,2,{1}} (* edges *)
}];


tally3x3x3[[All,2]]//Total
WeightedMean@tally3x3x3


Export["cube3.csv",tally3x3x3]


tallyMegaminx=CalculateOrders[{
{20,3,0}, (* corners*)
{30,2,0} (* edges *)
}];


tallyMegaminx[[All,2]]//Total
(WeightedData@@Transpose[tallyMegaminx])//Mean//N


Export["megaminx.csv",tallyMegaminx]


tallySuper4x4x4=CalculateOrders[{
{8,3}, (* corners*)
{24,1,{1}}, (* X-centers *)
{24,1} (* wings *)
}];


tallySuper4x4x4[[All,2]]//Total
24!*8!/2*3^7*24!
WeightedMean@tallySuper4x4x4


Export["cube4.csv",tallySuper4x4x4]


tallySuper5x5x5=CalculateOrders[{
{8,3}, (* corners*)
{12,2,{1}}, (* midges *)
{24,1,{1}}, (* X-centers *)
{24,1}, (* wings *)
{24,1,{1,2,3,4}} (* T-centers *)
}];


tallySuper5x5x5[[All,2]]//Total
24!*12!*2^11*24!*24!*8!*3^7/8
WeightedMean@tallySuper5x5x5


Export["cube5.csv",tallySuper5x5x5]


orbitsNxNxN[n_]:=Module[{l={},numWings=Floor[n/2-1],cornerIdx},
	l=Catenate[{l,
		Table[{24,1},{numWings}] (* wings *)
	}];
	AppendTo[l,{8,3}]; (* corner *)
	cornerIdx=numWings+1;
	l=Catenate[{l,
		Catenate@Table[
			If[
				i==j,
				{24,1,{cornerIdx}}, (* X-centers *)
				{24,1,{cornerIdx,i,j}} (* Oblique centers *)
			]
		,{i,numWings},{j,numWings}]
	}];
	If[OddQ[n],
		AppendTo[l,{12,2,{cornerIdx}}]; (* midge *)
		l=Catenate[{l,
			Table[
				{24,1,{cornerIdx,i}} (* T-centers *)
			,{i,numWings}]
		}];
	];
	l
]


CalculateOrders[orbitsNxNxN[3]]==tally3x3x3
CalculateOrders[orbitsNxNxN[4]]==tallySuper4x4x4
CalculateOrders[orbitsNxNxN[5]]==tallySuper5x5x5


Table[
WeightedMean@CalculateOrders[orbitsNxNxN[n]]
,{n,2,7}]
